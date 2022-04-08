const getAddress = () => window.location.pathname.split('/')[2];

const updateLoading = (status, tableId) => {
  if (status) {
    $(`#${tableId} > tbody`).addClass('blur');
    $('#pagination').addClass('hidden');
    $('#footer').addClass('hidden');
  } else {
    $(`#${tableId} > tbody`).removeClass('blur');
    $('#pagination').removeClass('hidden');
    $('#footer').removeClass('hidden');
  }
};

const datatableTxs = () => {
  const address = getAddress();

  $('#address-txs-table').DataTable({
    ...window.datatable.baseConfig,
    ajax: `/api/address/${address}/transactions`,
    columns:[
      { name: "age", data: 'timestamp', title: "Age", render: window.datatable.renderAge },
      { name: "timestamp", data: 'timestamp', title: "Date (UTC" + tzOffset + ")", render: window.datatable.renderTimestamp },
      { name: "txHash", data: 'txHash', title: "Transaction ID", className: "hash", render: window.datatable.renderCompactTxHash },
      { name: "blockHeight", title: "Block Height", render: window.datatable.renderBlockHeight },
      { name: "size", data: 'size', title: "Size", render: window.datatable.renderSize },
      { name: "fee", title: "Fee [sats]", className: "fee", render: window.datatable.renderFee },
      { name: "numInputs", data: 'numInputs', title: "Inputs" },
      { name: "numOutputs", data: 'numOutputs', title: "Outputs" },
      { name: "deltaSats", data: 'deltaSats', title: "Amount XEC", render: window.datatable.renderAmountXEC },
      { name: "token", title: "Amount Token", render: window.datatable.renderTokenAmountTicker },
      { name: 'responsive', render: () => '' },
    ],
  });

  params = window.state.getParameters();
  $('#address-txs-table').dataTable().api().page.len(params.txRows);
}

const datatableCashOutpoints = cashOutpoints => {
  $('#outpoints-table').DataTable({
    ...window.datatable.baseConfig,
    data: cashOutpoints,
    columns:[
      { name: "outpoint", className: "hash", render: window.datatable.renderOutpoint },
      { name: "block", render: window.datatable.renderOutpointHeight },
      { name: "xec", data: 'satsAmount', render: window.datatable.renderAmountXEC },
      { name: 'responsive', render: () => '' },
    ],
  });

  params = window.state.getParameters();
  $('#outpoints-table').dataTable().api().page.len(params.eCashOutpointsRows);
};

const datatableTokenOutpoints = tokenUtxos => {
  $('#address-token-outpoints-table').DataTable({
    ...window.datatable.baseConfig,
    data: tokenUtxos,
    columns:[
      { name: 'outpoint', className: "hash", render: window.datatable.renderOutpoint },
      { name: 'block', render: window.datatable.renderOutpointHeight },
      { name: 'amount', data: 'tokenAmount', render: window.datatable.renderTokenAmount },
      { name: 'dust', data: 'satsAmount', render: window.datatable.renderAmountXEC },
      { name: 'responsive', render: () => '' },
    ],
  });
};

const datatableTokenBalances = tokenBalances => {
  $('#address-token-balances-table').DataTable({
    ...window.datatable.baseConfig,
    data: tokenBalances,
    columns:[
      { name: 'amount', data: 'tokenAmount', render: window.datatable.renderToken },
      { name: 'ticker', data: 'token.tokenTicker' },
      { name: 'name', data: 'token.tokenName' },
      { name: 'dust', data: 'satsAmount', render: window.datatable.renderAmountXEC },
      { name: 'responsive', render: () => '' },
    ],
  });
};

$('#address-txs-table').on('xhr.dt', () => {
  updateLoading(false, 'address-txs-table');
});

$('#outpoints-table').on('xhr.dt', () => {
  updateLoading(false, 'outpoints-table');
});

const updateTable = (paginationRequest) => {
  const params = new URLSearchParams(paginationRequest).toString();
  const address = getAddress();

  updateLoading(true);
  $('#address-txs-table').dataTable().api().ajax.url(`/api/address/${address}/transactions?${params}`).load()
}

const goToPage = (event, page) => {
  event.preventDefault();
  reRenderPage({ page });
};

$(document).on('change', '[name*="-table_length"]', event => {
  reRenderPage({
    rows: event.target.value,
    page: 1,
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

  const paginationRequest = window.pagination.generatePaginationRequestOffset();
  updateTable(paginationRequest);

  const { currentPage, pageArray } = window.pagination.generatePaginationUIParams();
  window.pagination.generatePaginationUI(currentPage, pageArray);
};

$('#outpoints-table').on('init.dt', () => {
  updateLoading(false, 'outpoints-table');
});

$('#address-token-balances-table').on('init.dt', () => {
  updateLoading(false, 'address-token-balances-table');
});

$('#address-token-outpoints-table').on('init.dt', () => {
  updateLoading(false, 'address-token-outpoints-table');
});

$('.address__menu-tab').click(() => {
  setTimeout(() => {
    footerDynamicPositionFix();
  }, 20)
});

const getAddressBalances = () => {
  const address = getAddress();
  return fetch(`/api/address/${address}/balances`)
    .then(response => response.json())
    .then(response => response.data);
}

$(document).ready(() => {
  datatableTxs();
  getAddressBalances()
    .then(balances => {
      const cashBalance = balances.shift();
      const tokenBalances = balances;

      let tokenUtxos = []
      if (tokenBalances.length > 0) { 
        tokenUtxos = tokenBalances.reduce((acc, balance) => (
          [].concat(acc, balance.utxos))
        );
      }

      datatableCashOutpoints(cashBalance.utxos);
      datatableTokenOutpoints(tokenUtxos);
      datatableTokenBalances(tokenBalances);
    });

  $('.menu .item').tab({
    onVisible: tabPath => (
      reRenderPage({ currentTab: tabPath })
    )
  });

  reRenderPage()
});
