const getAddress = () => window.location.pathname.split('/')[2];

var isSatsTableLoaded = false;
function loadSatsTable() {
  if (!isSatsTableLoaded) {
    webix.ui({
      container: "sats-coins-table",
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
          header: "Height",
          adjust: true,
          template: function (row) {
            return '<a href="/block-height/' + row.blockHeight + '">' + renderInteger(row.blockHeight) + '</a>';
          },
        },
        {
          id: "amount",
          header: "XEC amount",
          adjust: true,
          template: function (row) {
            return renderSats(row.satsAmount) + ' XEC';
          },
        },
      ],
      autoheight: true,
      autowidth: true,
      data: addrBalances["main"].utxos,
    });
    isSatsTableLoaded = true;
  }
}

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
            const tokenTicker = addrBalances[tokenId].token?.tokenTicker;
            if (row.isMintBaton) {
              return 'Mint Baton ' + tokenTicker;
            } else {
              return renderAmount(row.tokenAmount, addrBalances[tokenId].token?.decimals) + ' ' + tokenTicker;
            }
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
  if (row.slpv2Sections.length == 0) {
    return '';
  }
  let sectionsHtml = '';
  for (const section of row.slpv2Sections) {
    sectionsHtml += '<span class="token-bubble" style="background-color: ' + section.tokenInfo.tokenColor + '">';
    sectionsHtml += renderAmount(section.stats.deltaTokens, section.tokenInfo.decimals);
    sectionsHtml += ' <a href="/tx/' + section.tokenInfo.tokenId + '">' + section.tokenInfo.tokenTicker + '</a> ';
    sectionsHtml += '</span>';
  }
  return sectionsHtml;
};

const updateTableLoading = (isLoading, tableId) => {
  if (isLoading) {
    $(`#${tableId} > tbody`).addClass('blur');
    $('#pagination').addClass('hidden');
    $('#footer').addClass('hidden');
  } else {
    $(`#${tableId} > tbody`).removeClass('blur');
    $('#pagination').removeClass('hidden');
    $('#footer').removeClass('hidden');
  }
};

const datatable = () => {
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
    order: [],
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
      { name: "blockHeight", title: "Height", render: renderBlockHeight },
      { name: "size", data: 'size', title: "Size", render: renderSize },
      { name: "fee", title: "Fee [sats]", className: "fee", render: renderFee },
      { name: "numInputs", data: 'numInputs', title: "Inputs" },
      { name: "numOutputs", data: 'numOutputs', title: "Outputs" },
      { name: "deltaSats", data: 'deltaSats', title: "Amount XEC", render: renderAmountXEC },
      { name: "token", title: "Amount Token", render: renderToken },
      { name: 'responsive', render: () => '' },
    ],
  });
}

$('#address-txs-table').on('xhr.dt', () => {
  updateTableLoading(false, 'address-txs-table');
});

const updateTable = (paginationRequest) => {
  const params = new URLSearchParams(paginationRequest).toString();
  const address = getAddress();

  updateTableLoading(true, 'address-txs-table');
  $('#address-txs-table').dataTable().api().ajax.url(`/api/address/${address}/transactions?${params}`).load()
}

const goToPage = (event, page) => {
  event.preventDefault();
  reRenderPage({ page });
};

$(document).on('change', '[name="address-txs-table_length"]', event => {
  reRenderPage({ rows: event.target.value, page: 1 });
});

const reRenderPage = params => {
  if (params) {
    window.state.updateParameters(params)
  }

  const paginationRequest = window.pagination.generatePaginationRequest();
  updateTable(paginationRequest);

  const { currentPage, pageArray } = window.pagination.generatePaginationUIParams();
  window.pagination.generatePaginationUI(currentPage, pageArray);
};

$(document).ready(() => {
  datatable();
  reRenderPage();
});
