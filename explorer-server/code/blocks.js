const DEFAULT_PAGE = 1;
const DEFAULT_ROWS_PER_PAGE = 100;
const DEFAULT_ORDER = 'desc';

// data table rendering utilities
const renderInt = (number) => {
  var fmt = Intl.NumberFormat('en-EN').format(number);
  var parts = fmt.split(',');
  var str = '';
  for (var i = 0; i < parts.length - 1; ++i) {
    str += '<span class="digit-sep">' + parts[i] + '</span>';
  }
  str += '<span>' + parts[parts.length - 1] + '</span>';
  return str;
}
const renderAge = (row) => moment(row.timestamp * 1000).fromNow();
const renderTemplate = (row) => '<a href="/block-height/' + row.height + '">' + renderInt(row.height) + '</a>';
const renderHash = (row) => `<a href="/block/${row.hash}">${minifyHash(row.hash)}</a>`;
const renderNumtTxs = (row) => renderInt(row.numTxs);
const renderSize = (row) => {
  if (row.size < 1024) {
    return row.size + ' B';
  } else if (row.size < 1024 * 1024) {
    return (row.size / 1000).toFixed(2) + ' kB';
  } else {
    return (row.size / 1000000).toFixed(2) + ' MB';
  }
};
const renderDifficulty = (row) => {
  const estHashrate = row.difficulty * 0xffffffff / 600;

  if (estHashrate < 1e12) {
    return (estHashrate / 1e9).toFixed(2) + ' GH/s';
  } else if (estHashrate < 1e15) {
    return (estHashrate / 1e12).toFixed(2) + ' TH/s';
  } else if (estHashrate < 1e18) {
    return (estHashrate / 1e15).toFixed(2) + ' PH/s';
  } else {
    return (estHashrate / 1e18).toFixed(2) + ' EH/s';
  }
};
const renderTimestamp = (row) => moment(row.timestamp * 1000).format('ll, LTS');


// state
const getBlockHeight = () => $('#pagination').data('last-block-height');

const getParameters = () => {
  const urlParams = new URLSearchParams(window.location.search);
  const page = validatePaginationInts(urlParams.get('page'), DEFAULT_PAGE) - 1;
  const humanPage = page + 1;
  const rows = validatePaginationInts(urlParams.get('rows'), DEFAULT_ROWS_PER_PAGE);
  const order = urlParams.get('order') || DEFAULT_ORDER;
  const start = parseInt(urlParams.get('start')) || 0;
  const end = parseInt(urlParams.get('end')) || getBlockHeight();

  return { page, humanPage, rows, order, start, end };
}

const updateParameters = params => {
  const path = window.location.pathname;
  const currentURLParams = Object.fromEntries(new URLSearchParams(window.location.search).entries());
  const newURLParams = new URLSearchParams({ ...currentURLParams, ...params });

  window.history.pushState('', document.title, `${path}?${newURLParams.toString()}`);
}

const updateLoading = (status) => {
  if (status) {
    $('.loader__container').removeClass('hidden');
    $('#pagination').addClass('hidden');
    $('#footer').addClass('hidden');
  } else {
    $('.loader__container').addClass('hidden');
    $('#pagination').removeClass('hidden');
    $('#footer').removeClass('hidden');
  }
};


// data fetching
const updateTable = (startPosition, endPosition) => {
  $$('blocks-table').clearAll();
  updateLoading(true);

  webix.ajax().get(`/api/blocks/${endPosition}/${startPosition}`)
    .then(res => {
      const results = res.json();
      $$('blocks-table').parse(results);
    });
}

// pagination
const validatePaginationInts = (value, fallback) => {
  parsedValue = parseInt(value);
  return isNaN(parsedValue) ? fallback : Math.max(parsedValue, 1);
}

const generatePaginationRequestParams = () => {
  const { page, rows, start, end } = getParameters();

  const startPosition = end - (page * rows);
  const endPosition = Math.max(startPosition - rows, start);

  return [ startPosition, endPosition ];
};

const determinePaginationSlots = lastPage => {
  let availableWidth = $('.ui.container').width();

  // pagination slot
  const padding = 2 * 16;
  const letter = 8;
  const tier1 = padding + 1 * letter;
  const tier2 = padding + 2 * letter;
  const tier3 = padding + 3 * letter;
  const tier4 = padding + 4 * letter;

  let predictedTier;
  if (lastPage > 0 && lastPage < 9) {
    predictedTier = tier1;
  } else if (lastPage > 9 && lastPage <= 99) {
    predictedTier = tier2;
  } else if (lastPage > 99 && lastPage <= 999) {
    predictedTier = tier3;
  } else if (lastPage > 999 && lastPage <= 9999) {
    predictedTier = tier4;
  }

  availableWidth -= tier1
  availableWidth -= predictedTier

  return Math.round((availableWidth) / predictedTier);
};

const findClosest = (haystack, needle) => (
  haystack.reduce((a, b) => (
    Math.abs(b - needle) < Math.abs(a - needle) ? b : a
  ))
);

const generatePaginationArray = (currentPage, max, slots) => {
  let increments;

  if (slots <= 6) {
    increments = [1, 100, 500, 1000, 2000, 4000];
  }
  else if (slots <= 10) {
    increments = [1, 10, 50, 100, 500, 1000, 2000, 4000];
  }
  else {
    increments = [1, 2, 10, 20, 50, 100, 500, 1000, 2000, 4000];
  }

  let pageArray = [];

  for (i = 0; i < Math.floor(slots / 2); i++) {
    const currentIncrement = increments[i];

    if (!currentIncrement || (currentPage - currentIncrement <= 1)) {
      break;
    }

    const value = currentPage - currentIncrement
    const precision = String(value).length - 1

    if (currentIncrement >= 10) {
      pageArray.push(parseFloat(value.toPrecision(precision)));
    } else {
      pageArray.push(value);
    }
  }

  pageArray = pageArray.reverse();
  if (currentPage != 1) { pageArray.push(currentPage) };

  const remainingSlots = slots - pageArray.length;
  for (i = 0; i < remainingSlots; i++) {
    const currentIncrement = increments[i];
    const value  = currentPage + currentIncrement;

    if (!currentIncrement || (value > max)) {
      break;
    }

    const precision = String(value).length - 1

    if (currentIncrement >= 10) {
      pageArray.push(parseFloat(value.toPrecision(precision)));
    } else {
      pageArray.push(value);
    }
  }

  if (currentPage == max) { pageArray.pop() };
  return pageArray;
};

const generatePaginationUIParams = () => {
  const { humanPage: currentPage, rows } = getParameters();
  const blockHeight = getBlockHeight();
  const lastPage = Math.ceil(blockHeight / rows);

  const slots = determinePaginationSlots(lastPage);

  const pageArray = generatePaginationArray(currentPage, lastPage, slots)
  pageArray.unshift(1)
  pageArray.push(lastPage)

  return { currentPage, pageArray };
};

const generatePaginationUI = (currentPage, pageArray) => {
  const path = window.location.pathname;

  // DOM building blocks
  const activeItem = (number) => `<a class="item active">${number}</a>`;
  const item = (number) => `<a class="item" href="${path}?page=${number}" onclick="goToPage(event, ${number})">${number}</a>`;

  let pagination = '';
  pagination += '<div class="ui pagination menu">';

  pageArray.forEach((pageNumber, i) => {
    if (pageNumber === currentPage) {
      pagination += activeItem(pageNumber)
      return;
    }

    pagination += item(pageNumber);
  });

  pagination += '</div>';

  $('#pagination').html(pagination);
};

// UI actions
const goToPage = (event, page) => {
  event.preventDefault();
  reRenderPage({ page });
};


// UI presentation elements

const dataTable = () => {
  const tzOffset = new Date().getTimezoneOffset();
  let tzString;

  if (tzOffset < 0) {
    tzString = moment.utc(moment.duration(-tzOffset, 'minutes').asMilliseconds()).format('+HH:mm');
  } else {
    tzString = moment.utc(moment.duration(tzOffset, 'minutes').asMilliseconds()).format('-HH:mm');
  }

  webix.ui({
    id: "blocks-table",
    container: "blocks-table",
    view: "datatable",
    css: 'datatable__block_listing',
    hover: 'datatable__block_listing-hover',
    scroll: false,
    columns:[
      { adjust: true, id: "age", header: "Age", template: renderAge },
      { width: 80, id: "height", header: "Height", template: renderTemplate },
      { adjust: true, id: "numTxs", header: "Transactions", template: renderNumtTxs },
      { fillspace: true, id: "hash", header: "Block Hash", css: "hash", template: renderHash },
      { width: 100, id: "size", header: "Size", css: "size", template: renderSize },
      { width: 130, id: "difficulty", header: "Est. Hashrate", template: renderDifficulty },
      { adjust: true, id: "timestamp", header: "Date (UTC" + tzString + ")", template: renderTimestamp },
    ],
    autoheight: true,
    on: {
      onAfterLoad: () => updateLoading(false),
    }
  });	
}


// events
$(window).resize(() => {
  const { currentPage, pageArray } = generatePaginationUIParams();
  generatePaginationUI(currentPage, pageArray);
});


// Basically a fake refresh, dynamically updates everything
// according to new params
// updates: URL, table and pagination
const reRenderPage = params => {
  if (params) {
    updateParameters(params)
  }

  const [ startPosition, endPosition ] = generatePaginationRequestParams();
  updateTable(startPosition, endPosition);

  const { currentPage, pageArray } = generatePaginationUIParams();
  generatePaginationUI(currentPage, pageArray);
};


// main
webix.ready(() => {
  // init all UI elements
  dataTable();

  // global state update
  reRenderPage();
});
