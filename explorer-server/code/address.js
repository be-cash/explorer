const getAddress = () => window.location.pathname.split('/')[2];

const updateLoading = (status, tableId) => {
  if (status) {
    $(`#${tableId} > tbody`).addClass('blur');
    $('#pagination').addClass('visibility-hidden');
    $('#footer').addClass('visibility-hidden');
  } else {
    $(`#${tableId} > tbody`).removeClass('blur');
    $('#pagination').removeClass('visibility-hidden');
    $('#footer').removeClass('visibility-hidden');
  }
};

const datatableTxs = () => {
  $('#address-txs-table').DataTable({
    ...window.datatable.baseConfig,
    columnDefs: [
      ...window.datatable.baseConfig.columnDefs,
      {
        targets: -2,
        createdCell: (td, _cellData, row) => {
          const isNegative = Math.sign(row.deltaSats);
          if (isNegative) {

            $(td).css('background-color', 'red')
          }
            $(td).css('background-color', 'lightgreen')
        },
      },
    ],
    columns:[
      { name: "age", data: 'timestamp', title: "Age", render: window.datatable.renderAge },
      { name: "timestamp", data: 'timestamp', title: "Date (UTC" + tzOffset + ")", render: window.datatable.renderTimestamp },
      { name: "txHash", data: 'txHash', title: "Transaction ID", className: "hash", render: window.datatable.renderCompactTxHash },
      { name: "blockHeight", title: "Block Height", render: window.datatable.renderBlockHeight },
      { name: "size", data: 'size', title: "Size", render: window.datatable.renderSize },
      { name: "fee", title: "Fee [sats]", className: "fee", render: window.datatable.renderFee },
      { name: "numInputs", data: 'numInputs', title: "Inputs" },
      { name: "numOutputs", data: 'numOutputs', title: "Outputs" },
      { name: "amount", data: 'deltaSats', title: "Amount", className: 'address__txs-table__amount-cell', render: window.datatable.renderOutput },
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

$('#address-txs-table').on('init.dt', () => {
  updateLoading(false, 'address-txs-table');
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

const getAddressTransactions = () => {
  const address = getAddress();
  return fetch(`/api/address/${address}/transactions`)
    .then(response => response.json())
    .then(response => response.data);
}

const getAddressStatistics = () => {
  const address = getAddress();
  return fetch(`/api/address/${address}/statistics`)
    .then(response => response.json())
    .then(response => response.data);
}

const cubeSelectors = ['.cash-address', '.token-address', '.legacy-address']
const cubeRotate = (motion, next) => {
  $('.address__qr-code-wrapper')
    .shape('set next side', cubeSelectors[next - 1])
    .shape(`flip ${motion}`);
};

$('#address__selector tr').click(function() {
  const cubeTotalSides = 3
  const previousSide = $('.address__qr-code-wrapper').data('cube-active-side');
  const desiredSide = $(this).data('cube-side')

  if (previousSide === desiredSide) {
    return;
  }

  const diff  = desiredSide - previousSide;
  const absoluteDiff  = Math.abs(diff);
  const motion = diff < 0 ? 'down' : 'up';
  const isAnimating = $('.address__qr-code-wrapper').shape('is animating');

  $(this).siblings().removeClass('left marked')
  $(this).addClass('left marked')

  if (isAnimating) {
    setTimeout(function () {
      $('.address__qr-code-wrapper').shape('reset');
      $('.address__qr-code-wrapper').shape('refresh');
      cubeRotate(motion, desiredSide)
    }, 300);
    $('.address__qr-code-wrapper').data('cube-active-side', desiredSide);
    return;
  }

  let currentSide = previousSide;
  for (i = 0; i < absoluteDiff; i++) {
    const n = motion === 'down' ? -1 : 1;
    const next = ((currentSide + n) % (cubeTotalSides + 1) + (cubeTotalSides + 1)) % (cubeTotalSides + 1);
    console.log(next)
    console.log(cubeSelectors[next - 1])
    currentSide = next

    const isAnimating = $('.address__qr-code-wrapper').shape('is animating');

    if (isAnimating) {
      setTimeout(function () {
        cubeRotate(motion, next)
      }, 300);
    } else {
      cubeRotate(motion, next)
    }
  }

  $('.address__qr-code-wrapper').data('cube-active-side', desiredSide);
});

$(document).ready(() => {
  datatableTxs();

  getAddressStatistics()
    .then(statistics => {
      const {
        totalTxsReceived,
        totalTxsSent,
        totalUtxos,
        firstBalanceChange,
        lastBalanceChange,
      } = statistics;

      const firstTimestamp = firstBalanceChange ? moment(firstBalanceChange * 1000).fromNow() : 'Never';
      const lastTimestamp = firstBalanceChange ? moment(lastBalanceChange * 1000).fromNow() : 'Never';

      $('#address-total-received').text(totalTxsReceived);
      $('#address-total-sent').text(totalTxsSent);
      $('#address-total-utxos').text(totalUtxos);
      $('#address-first-balance-change').text(firstTimestamp);
      $('#address-last-balance-change').text(lastTimestamp);
    });

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

  $('.address__qr-code-wrapper').shape({ duration: 250 });

  const cashAddress = $('#qr-cash-address');
  const tokenAddress = $('#qr-token-address');
  const legacyAddress = $('#qr-legacy-address');

  QrCreator.render({
    text: cashAddress.data('address'),
    radius: 0.5, // 0.0 to 0.5
    ecLevel: 'H', // L, M, Q, H
    fill: '#FFFFFF', // foreground color
    background: null, // color or null for transparent
    size: 290// in pixels
  }, cashAddress.get(0));

  QrCreator.render({
    text: tokenAddress.data('address'),
    radius: 0.5, // 0.0 to 0.5
    ecLevel: 'H', // L, M, Q, H
    fill: '#FFFFFF', // foreground color
    background: null, // color or null for transparent
    size: 290// in pixels
  }, tokenAddress.get(0));

  QrCreator.render({
    text: legacyAddress.data('address'),
    radius: 0.5, // 0.0 to 0.5
    ecLevel: 'H', // L, M, Q, H
    fill: '#FFFFFF', // foreground color
    background: null, // color or null for transparent
    size: 290// in pixels
  }, legacyAddress.get(0));

  reRenderPage()
});
