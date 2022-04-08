const DEFAULT_ROWS_PER_PAGE = 100;


var tzOffset;

{
  var offsetMinutes = new Date().getTimezoneOffset();
  if (offsetMinutes < 0) {
    tzOffset = moment.utc(moment.duration(-offsetMinutes, 'minutes').asMilliseconds()).format('+HH:mm');
  } else {
    tzOffset = moment.utc(moment.duration(offsetMinutes, 'minutes').asMilliseconds()).format('-HH:mm');
  }
}

function formatByteSize(size) {
  if (size < 1024) {
    return size + ' B';
  } else if (size < 1024 * 1024) {
    return (size / 1000).toFixed(2) + ' kB';
  } else {
    return (size / 1000000).toFixed(2) + ' MB';
  }
}

function renderInteger(number) {
  var fmt = Intl.NumberFormat('en-EN').format(number);
  var parts = fmt.split(',');
  var str = '';
  for (var i = 0; i < parts.length; ++i) {
    const classSep = i == parts.length - 1 ? '' : "digit-sep";
    if (i >= 2) {
      str += '<small class="' + classSep + '">' + parts[i] + '</small>';
    } else {
      str += '<span class="' + classSep + '">' + parts[i] + '</span>';
    }
  }
  return str;
}

function renderAmount(baseAmount, decimals) {
  if (decimals === 0) {
    return renderInteger(baseAmount);
  }
  var factor = Math.pow(10, decimals);
  var humanAmount = baseAmount / factor;
  var fmt = humanAmount.toFixed(decimals);
  var parts = fmt.split('.');
  var integerPart = parseInt(parts[0]);
  var fractPart = parts[1];
  var numFractSections = Math.ceil(decimals / 3);
  var fractRendered = '';
  var allZeros = true;
  for (var sectionIdx = numFractSections - 1; sectionIdx >= 0; --sectionIdx) {
    var section = fractPart.substr(sectionIdx * 3, 3);
    if (parseInt(section) !== 0)
      allZeros = false;
    var classes =
      (allZeros ? 'zeros ' : '') +
      (sectionIdx != numFractSections - 1 ? 'digit-sep ' : '');
    fractRendered = '<small class="' + classes + '">' + section + '</small>' + fractRendered;
  }
  return renderInteger(integerPart) + '.' + fractRendered;
}

function renderSats(sats) {
  var coins = sats / 100;
  var fmt = coins.toFixed('2');
  var parts = fmt.split('.');
  var integerPart = parseInt(parts[0]);
  var fractPart = parts[1];
  var fractZero = fractPart === '00';

  if (fractZero) {
    return renderInteger(integerPart);
  } else {
    return renderInteger(integerPart) + '.<small>' + fractPart + '</small>';
  }
}

var regHex32 = /^[0-9a-fA-F]{64}$/
function searchBarChange() {
  if (event.key == 'Enter') {
    return searchButton();
  }
  var search = $('#search-bar').val();
  if (search.match(regHex32) !== null) {
    location.href = '/search/' + search;
  }
}

function searchButton() {
  var search = $('#search-bar').val();
  location.href = '/search/' + search;
}

function toggleTransactionScriptData() {
  $('.tx-transaction__script-data').each(function () {
    $(this).toggleClass('display-none');
  });
}

function minifyHash(hash) {
  return `${hash.slice(0, 25)}...${hash.slice(39, 64)}`;
}

const generateRange = (start, end) => [...Array(end - start + 1)].map((_, i) => start + i);

const findClosest = (haystack, needle) => (
  haystack.reduce((a, b) => (
    Math.abs(b - needle) < Math.abs(a - needle) ? b : a
  ))
);

const scrollToBottom = () => {
  const pageHeight = $(document).height()-$(window).height();
  $("html, body").animate({ scrollTop: pageHeight - 50 }, 250);
};

(function(datatable, $) {
  const renderTxHash = hash => {
    return hash.substr(0, 10) + '&hellip;' + hash.substr(60, 4)
  }

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

  datatable.baseConfig = {
    searching: false,
    lengthMenu: [50, 100, 250, 500, 1000],
    pageLength: DEFAULT_ROWS_PER_PAGE,
    language: {
      loadingRecords: '',
      zeroRecords: '',
      emptyTable: '',
    },
    order: [ [ 1, 'desc' ] ],
    responsive: {
        details: {
            type: 'column',
            target: -1
        }
    },
    columnDefs: [ {
        className: 'dtr-control',
        orderable: false,
        targets: -1
    } ],
  };

  datatable.renderAge = timestamp => (
    moment(timestamp * 1000).fromNow()
  );

  datatable.renderAddressAge = timestamp => {
    if (timestamp == 0) {
      return '<div class="ui gray horizontal label">Mempool</div>';
    }
    return moment(timestamp * 1000).fromNow();
  };

  datatable.renderTemplate = height => (
    `<a href="/block-height/${height}">${renderInt(height)}</a>`
  );

  datatable.renderBlockHash = (hash, _type, _row, meta) => {
    const api = new $.fn.dataTable.Api( meta.settings );
    const isHidden = !api.column(4).responsiveHidden();
    let minifiedHash = minifyHash(hash)

    if (isHidden) {
      minifiedHash = minifiedHash.split('.')[0];
    }

    return `<a href="/block/${hash}">${minifiedHash}</a>`
  };

  datatable.renderCompactTxHash = hash => (
    `<a href="/tx/${hash}">${renderTxHash(hash)}</a>`
  );

  datatable.renderTxHash = hash => (
    `<a href="/tx/${hash}">${hash}</a>`
  );

  datatable.renderNumtTxs = numTxs => (
    renderInt(numTxs)
  );

  datatable.renderSize = size => {
    if (size < 1024) {
      return size + ' B';
    } else if (size < 1024 * 1024) {
      return (size / 1000).toFixed(2) + ' kB';
    } else {
      return (size / 1000000).toFixed(2) + ' MB';
    }
  };

  datatable.renderDifficulty = difficulty => {
    const estHashrate = difficulty * 0xffffffff / 600;

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

  datatable.renderTimestamp = timestamp => (
    moment(timestamp * 1000).format('ll, LTS')
  );

  datatable.renderAddressTimestamp = timestamp => {
    if (timestamp == 0) {
      return '<div class="ui gray horizontal label">Mempool</div>';
    }
    return moment(timestamp * 1000).format('ll, LTS');
  };


  datatable.renderFee = (_value, _type, row) => {
    if (row.isCoinbase) {
      return '<div class="ui green horizontal label">Coinbase</div>';
    }

    const fee = renderInteger(row.satsInput - row.satsOutput);
    const feePerByte = renderInteger(
      Math.round(((row.satsInput - row.satsOutput) / row.size) * 1000)
    );

    let markup = '';
    markup += `<span>${fee}</span>`
    markup += `<span class="fee-per-byte">(${feePerByte} /kB)</span>`

    return markup;
  };

  datatable.renderOutput = (_value, _type, row) => {
    if (row.token) {
      const ticker = `<a href="/tx/${row.token.tokenId}">${row.token.tokenTicker}</a>`;
      return `${renderAmount(row.tokenOutput, row.token.decimals)} ${ticker}`;
    }

    return `${renderSats(row.satsOutput)} XEC`;
  };

  datatable.renderOutpoint = (_value, _type, row) => {
    const { txHash, outIdx } = row;
    const label = row.isCoinbase ? '<div class="ui green horizontal label">Coinbase</div>' : '';
    return `<a href="/tx/${txHash}">${txHash}:${outIdx}${label}</a>`;
  };

  datatable.renderOutpointHeight = (_value, _type, row) => {
    const { blockHeight } = row;
    return `<a href="/block-height/${blockHeight}">${renderInteger(blockHeight)}</a>`;
  };

  datatable.renderBlockHeight = (_value, _type, row) => {
    if (row.timestamp == 0) {
      return '<div class="ui gray horizontal label">Mempool</div>';
    }
    return `<a href="/block-height/${row.blockHeight}">${renderInteger(row.blockHeight)}</a>`;
  };

  datatable.renderAmountXEC = sats => (
    `${renderSats(sats)} XEC`
  );

  datatable.renderTokenAmountTicker = (_value, _type, row) => {
    if (row.token !== null) {
      const ticker = `<a href="/tx/${row.token.tokenId}">${row.token.tokenTicker}</a>`;
      return `${renderAmount(row.tokenOutput, row.token.decimals)} ${ticker}`;
    }

    return '';
  };

  datatable.renderTokenAmount = (_value, _type, row) => (
    renderAmount(row.tokenAmount, row.token.decimals)
  );
}(window.datatable = window.datatable || {}, jQuery));

(function(state, $) {
  const DEFAULT_PAGE = 1;
  const DEFAULT_ROWS_PER_PAGE = 100;
  const DEFAULT_ORDER = 'desc';

  const validatePaginationInts = (value, fallback) => {
    parsedValue = parseInt(value);
    return isNaN(parsedValue) ? fallback : Math.max(parsedValue, 1);
  };

  const handleTabUpdate = params => {
    if ((!params.page && !params.rows) || !params.currentTab) {
      return params;
    }

    switch (params.currentTab) {
      case 'transactions':
        params.txp = validatePaginationInts(params.page, DEFAULT_PAGE);
        params.txr = validatePaginationInts(params.rows, DEFAULT_ROWS_PER_PAGE);
        break;

      case 'ecash-outpoints':
        params.ecop = validatePaginationInts(params.page, DEFAULT_PAGE);
        params.ecor = validatePaginationInts(params.rows, DEFAULT_ROWS_PER_PAGE);
        break;

      case 'etoken-balances':
        params.etbp = validatePaginationInts(params.page, DEFAULT_PAGE);
        params.etbr = validatePaginationInts(params.rows, DEFAULT_ROWS_PER_PAGE);
        break;

      case 'etoken-outpoints':
        params.etop = validatePaginationInts(params.page, DEFAULT_PAGE);
        params.etor = validatePaginationInts(params.rows, DEFAULT_ROWS_PER_PAGE);
        break;
    }

    delete params.page;
    delete params.rows;

    return params;
  };

  const handleTabGet = params => {
    if (!params.currentTab) {
      return params;
    }

    switch (params.currentTab) {
      case 'transactions':
        params.page = params.txp;
        params.rows = params.txr;
        break;

      case 'ecash-outpoints':
        params.page = params.ecop;
        params.rows = params.ecor;
        break;

      case 'etoken-balances':
        params.page = params.etbp;
        params.rows = params.etbr;
        break;

      case 'etoken-balances':
        params.page = params.etor;
        params.rows = params.etop;
        break;
    }

    params.page = validatePaginationInts(params.page, DEFAULT_PAGE)
    params.rows = validatePaginationInts(params.rows, DEFAULT_ROWS_PER_PAGE)

    return params;
  };

  state.getPaginationTotalEntries = () => $('#pagination').data('total-entries');

  state.getParameters = () => {
    let urlParams;

    urlParams = Object.fromEntries(new URLSearchParams(window.location.search).entries());
    urlParams = handleTabGet(urlParams);

    const page = validatePaginationInts(urlParams.page, DEFAULT_PAGE);
    const rows = validatePaginationInts(urlParams.rows, DEFAULT_ROWS_PER_PAGE);
    const order = urlParams.order || DEFAULT_ORDER;
    const start = parseInt(urlParams.start) || 0;
    const end = parseInt(urlParams.end) || state.getPaginationTotalEntries();

    return { ...urlParams, page, rows, order, start, end };
  }

  state.updateParameters = params => {
    let newParams;

    const path = window.location.pathname;
    const currentURLParams = Object.fromEntries(new URLSearchParams(window.location.search).entries());
    newParams = { ...currentURLParams, ...params }

    if (newParams.currentTab) {
      newParams = handleTabUpdate(newParams)
    }

    const newURLParams = new URLSearchParams(newParams);
    window.history.pushState('', document.title, `${path}?${newURLParams.toString()}`);

    return Object.fromEntries(newURLParams.entries());
  }

  state.updateLoading = status => {
    if (status) {
      $('.loader__container').removeClass('display-none');
      $('#pagination').addClass('visibility-hidden');
      $('#footer').addClass('visibility-hidden');
    } else {
      $('.loader__container').addClass('display-none');
      $('#pagination').removeClass('visibility-hidden');
      $('#footer').removeClass('visibility-hidden');
    }
  };
}(window.state = window.state || {}, jQuery));


(function(pagination, $) {
  const activeItem = '<a class="item active" href="HREF" onclick="goToPage(event, NUMBER)">NUMBER</a>';
  const disabledItem = '<div class="disabled item">...</div>';
  const item = '<a class="item" href="HREF" onclick="goToPage(event, NUMBER)">NUMBER</a>';

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
    if (lastPage > 0 && lastPage < 10) {
      predictedTier = tier1;
    } else if (lastPage > 9 && lastPage < 100) {
      predictedTier = tier2;
    } else if (lastPage > 99 && lastPage < 1000) {
      predictedTier = tier3;
    } else if (lastPage > 999 && lastPage <= 10000) {
      predictedTier = tier4;
    }

    availableWidth -= tier1
    availableWidth -= predictedTier

    return Math.round((availableWidth) / predictedTier);
  };

  pagination.generatePaginationRequestRange = () => {
    const { page, rows, start, end } = window.state.getParameters();

    const startPosition = end - (page * rows);
    const endPosition = Math.max(startPosition - rows, start);

    return [ startPosition, endPosition ];
  };

  pagination.generatePaginationRequestOffset = () => {
    const { page, rows } = window.state.getParameters();

    const offset = (page * rows) - rows;
    const take = rows;

    return { offset, take };
  };

  const generatePaginationArray = (currentPage, max, slots) => {
    if (slots > max) {
      return [...Array(max).keys()].slice(1).map(x => ++x);
    }

    let increments;
    let pageArray = [];

    if (slots <= 6) {
      increments = [1, 100, 500, 1000, 2000, 4000];
    }
    else if (slots <= 10) {
      increments = [1, 10, 50, 100, 500, 1000, 2000, 4000];
    }
    else {
      increments = [1, 2, 10, 50, 100, 500, 1000, 2000, 4000];
    }

    for (i = 0; i < Math.floor(slots / 2); i++) {
      const currentIncrement = increments[i];

      if (!currentIncrement || (currentPage - currentIncrement <= 1)) {
        break;
      }

      const value = currentPage - currentIncrement
      const precision = String(value).length - 1

      if (currentIncrement >= 10 && precision) {
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

      if (currentIncrement >= 10 && precision) {
        const round = parseFloat(value.toPrecision(precision))

        if (round >= max) {
          break;
        }

        pageArray.push(round);
      } else {
        pageArray.push(value);
      }
    }

    if (currentPage == max) { pageArray.pop() };

    if (max < 50000 && (slots - pageArray.length) > 10) {

      let index;
      const indexRound = pageArray.findIndex(x => !(x % 10));
      const indexPage = pageArray.indexOf(currentPage)

      if (indexRound <= 0) {
        index = 1
      } else if (indexRound > indexPage && currentPage > 10) {
        index = indexPage - 2
      } else {
        index = indexRound
      }

      const extension = [...Array(9).keys()].map(x => ++x);

      if (pageArray[index] != 10) {
        extension.push(10)
      }

      pageArray = pageArray.slice(index);
      pageArray = extension.concat(pageArray)
      pageArray.shift()
    }
    return pageArray;
  };

  pagination.generatePaginationUIParams = () => {
    const { page: currentPage, rows } = window.state.getParameters();
    const totalEntries = window.state.getPaginationTotalEntries();
    const lastPage = Math.ceil(totalEntries / rows);

    if (lastPage === 1) {
      return {};
    }

    const slots = determinePaginationSlots(lastPage);

    const pageArray = generatePaginationArray(currentPage, lastPage, slots)
    pageArray.unshift(1)

    if (lastPage !== pageArray.slice(-1)[0]) {
      pageArray.push(lastPage)
    }

    return { currentPage, pageArray };
  };

  pagination.generatePaginationUI = (currentPage, pageArray) => {
    if (!pageArray) {
      return;
    }

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

}(window.pagination = window.pagination || {}, jQuery));
