const getAddress = () => window.location.pathname.split('/')[2];

const renderOutpoint = (_value, _type, row) => {
  const { txHash, outIdx } = row;
  const label = row.isCoinbase ? '<div class="ui green horizontal label">Coinbase</div>' : '';
  return `<a href="/tx/${txHash}">${txHash}:${outIdx} ${label}</a>`;
};

const renderOutpointHeight = (_value, _type, row) => {
  const { blockHeight } = row;
  return `<a href="/block-height/${blockHeight}">${renderInteger(blockHeight)}</a>`;
};

const renderXEC = sats => `${renderSats(sats)} XEC`;

var isTokenTableLoaded = {};
function loadTokenTable(tokenId) {
  if (!isTokenTableLoaded[tokenId]) {
    webix.ui({
      container: "tokens-coins-table-" + tokenId,
      view: "datatable",
      columns:[
        {
          id: "outpoint",
          header: "Outpoint",
          css: "hash",
          adjust: true,
          template: function (row) {
            return '<a href="/tx/' + row.txHash + '">' + 
              row.txHash + ':' + row.outIdx +
              (row.isCoinbase ? '<div class="ui green horizontal label">Coinbase</div>' : '') +
              '</a>';
          },
        },
        {
          id: "blockHeight",
          header: "Block Height",
          adjust: true,
          template: function (row) {
            return '<a href="/block-height/' + row.blockHeight + '">' + renderInteger(row.blockHeight) + '</a>';
          },
        },
        {
          id: "tokenAmount",
          header: addrBalances[tokenId].token?.tokenTicker + " amount",
          adjust: true,
          template: function (row) {
            return renderAmount(row.tokenAmount, addrBalances[tokenId].token?.decimals) + ' ' + addrBalances[tokenId].token?.tokenTicker;
          },
        },
        {
          id: "satsAmount",
          header: "XEC amount",
          adjust: true,
          template: function (row) {
            return renderSats(row.satsAmount) + ' XEC';
          },
        },
      ],
      autoheight: true,
      autowidth: true,
      data: addrBalances[tokenId].utxos,
    });
    isTokenTableLoaded[tokenId] = true;
  }
}

const renderAge = timestamp => {
  if (timestamp == 0) {
    return '<div class="ui gray horizontal label">Mempool</div>';
  }
  return moment(timestamp * 1000).fromNow();
};

const renderTimestamp = timestamp => {
  if (timestamp == 0) {
    return '<div class="ui gray horizontal label">Mempool</div>';
  }
  return moment(timestamp * 1000).format('ll, LTS');
};

const renderTxID = txHash => {
  return '<a href="/tx/' + txHash + '">' + renderTxHash(txHash) + '</a>';
};

const renderBlockHeight = (_value, _type, row) => {
  if (row.timestamp == 0) {
    return '<div class="ui gray horizontal label">Mempool</div>';
  }
  return '<a href="/block-height/' + row.blockHeight + '">' + renderInteger(row.blockHeight) + '</a>';
};

const renderSize = size => formatByteSize(size);

const renderFee = (_value, _type, row) => {
  if (row.isCoinbase) {
    return '<div class="ui green horizontal label">Coinbase</div>';
  }

  const fee = renderInteger(row.stats.satsInput - row.stats.satsOutput);
  let markup = '';

  markup += `<span>${fee}</span>`
  markup += `<span class="fee-per-byte">&nbsp(${renderFeePerByte(_value, _type, row)})</span>`

  return markup;
};

const renderFeePerByte = (_value, _type, row) => {
  if (row.isCoinbase) {
    return '';
  }
  const fee = row.stats.satsInput - row.stats.satsOutput;
  const feePerByte = fee / row.size;
  return renderInteger(Math.round(feePerByte * 1000)) + '/kB';
};

const renderAmountXEC = (_value, _type, row) => renderSats(row.stats.deltaSats) + ' XEC';

const renderToken = (_value, _type, row) => {
  if (row.token !== null) {
    var ticker = ' <a href="/tx/' + row.token.tokenId + '">' + row.token.tokenTicker + '</a>';
    return renderAmount(row.stats.deltaTokens, row.token.decimals) + ticker;
  }
  return '';
};

const updateLoading = (status, tableId) => {
  if (status) {
    $(`#${tableId} > tbody`).addClass('blur');
    $('.loader__container--fullpage').removeClass('hidden');
    $('#pagination').addClass('hidden');
    $('#footer').addClass('hidden');
  } else {
    $(`#${tableId} > tbody`).removeClass('blur');
    $('.loader__container--fullpage').addClass('hidden');
    $('#pagination').removeClass('hidden');
    $('#footer').removeClass('hidden');
  }
};

const datatableTxs = () => {
  const address = getAddress();

  $('#address-txs-table').DataTable({
    searching: false,
    lengthMenu: [50, 100, 200],
    pageLength: DEFAULT_ROWS_PER_PAGE,
    language: {
      loadingRecords: '',
      zeroRecords: '',
      emptyTable: '',
    },
    ajax: `/api/address/${address}/transactions`,
    order: [ ],
    responsive: {
        details: {
            type: 'column',
            target: -1
        }
    },
    columnDefs: [ {
        className: 'dtr-control',
        orderable: false,
        targets:   -1
    } ],
    columns:[
      { name: "age", data: 'timestamp', title: "Age", render: renderAge },
      { name: "timestamp", data: 'timestamp', title: "Date (UTC" + tzOffset + ")", render: renderTimestamp },
      { name: "txHash", data: 'txHash', title: "Transaction ID", className: "hash", render: renderTxID },
      { name: "blockHeight", title: "Block Height", render: renderBlockHeight },
      { name: "size", data: 'size', title: "Size", render: renderSize },
      { name: "fee", title: "Fee [sats]", className: "fee", render: renderFee },
      { name: "numInputs", data: 'numInputs', title: "Inputs" },
      { name: "numOutputs", data: 'numOutputs', title: "Outputs" },
      { name: "deltaSats", data: 'deltaSats', title: "Amount XEC", render: renderAmountXEC },
      { name: "token", title: "Amount Token", render: renderToken },
      { name: 'responsive', render: () => '' },
    ],
  });

  const { rows } = window.state.getParameters('transactions');
  $('#address-txs-table').dataTable().api().page.len(rows);
}

const datatableOutpoints = () => {
  const address = getAddress();

  $('#outpoints-table').DataTable({
    searching: false,
    lengthMenu: [50, 100, 250, 500, 1000],
    pageLength: DEFAULT_ROWS_PER_PAGE,
    language: {
      loadingRecords: '',
      zeroRecords: '',
      emptyTable: '',
    },
    ajax: {
      url: `/api/address/${address}/balances`,
      dataSrc: response => {
        window.state.setPaginationTotalEntries('outpoints', response.data['main'].utxos.length)
        reRenderPage();

        return response.data['main'].utxos;
      }
    },
    order: [ ],
    responsive: {
        details: {
            type: 'column',
            target: -1
        }
    },
    columnDefs: [ {
        className: 'dtr-control',
        orderable: false,
        targets:   -1
    } ],
    columns:[
      { name: "outpoint", className: "hash", render: renderOutpoint },
      { name: "block", render: renderOutpointHeight },
      { name: "xec", data: 'satsAmount', render: renderXEC },
      { name: 'responsive', render: () => '' },
    ],
  });

  const { rows } = window.state.getParameters('outpoints');
  $('#outpoints-table').dataTable().api().page.len(rows);
};

$('#address-txs-table').on('xhr.dt', () => {
  updateLoading(false, 'address-txs-table');
});

$('#outpoints-table').on('xhr.dt', () => {
  updateLoading(false, 'outpoints-table');
});

$('#outpoints-table').on('init.dt', () => {
  const { rows, page } = window.state.getParameters('outpoints');
  updateLoading(true, 'address-txs-table');

  $('#outpoints-table').dataTable().api().page.len(rows);
  $('#outpoints-table').DataTable().page(page).draw('page');
});

const updateTransactionsTable = paginationRequest => {
  const params = new URLSearchParams(paginationRequest).toString();
  const address = getAddress();

  updateLoading(true, 'address-txs-table');
  $('#address-txs-table').dataTable().api().ajax.url(`/api/address/${address}/transactions?${params}`).load()
}

const updateOutpointsTable = paginationRequest => {
  const { page } = paginationRequest;
  $('#outpoints-table').DataTable().page(page).draw('page');
}

const goToPage = (event, page) => {
  event.preventDefault();
  reRenderPage({ page });
};

$(document).on('change', '[name*="-table_length"]', event => {
  reRenderPage({
    rows: event.target.value,
    page: 0,
  });
});

const reRenderPage = params => {
  if (params) {
    params = window.state.updateParameters(params);
  } else {
    params = window.state.getParameters();

    if (!params.currentTab) {
      window.state.updateParameters({ currentTab: 'transactions' });
    }
  }

  if (params.currentTab) {
    $('.menu .item').tab('change tab', params.currentTab);
  }

  const paginationRequest = window.pagination.generatePaginationRequest();

  if (params.currentTab == 'transactions') {
    updateTransactionsTable(paginationRequest);
  }
  else if (params.currentTab == 'outpoints') {
    updateOutpointsTable(paginationRequest)
  }

  const { currentPage, pageArray } = window.pagination.generatePaginationUIParams();
  window.pagination.generatePaginationUI(currentPage, pageArray);
};

$(document).ready(() => {
  datatableTxs();
  datatableOutpoints();

  $('.menu .item').tab({
    onVisible: tabPath => (
      reRenderPage({ currentTab: tabPath })
    )
  });

  reRenderPage();
});
