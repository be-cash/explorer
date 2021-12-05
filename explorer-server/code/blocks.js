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


// main
webix.ready(() => {
  // init all UI elements
  dataTable();

  const [ startPosition, endPosition ] = generatePaginationRequestParams();
  updateTable(startPosition, endPosition);
});
