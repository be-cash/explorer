const updateLoading = (status) => {
  if (status) {
    $('#txs-table > tbody').addClass('blur');
    $('.loader__container--fullpage').removeClass('visibility-hidden');
    $('#pagination').addClass('visibility-hidden');
    $('#footer').addClass('visibility-hidden');
  } else {
    $('#txs-table > tbody').removeClass('blur');
    $('.loader__container--fullpage').addClass('visibility-hidden');
    $('#pagination').removeClass('visibility-hidden');
    $('#footer').removeClass('visibility-hidden');
  }
};


// UI actions
const goToPage = (event, page) => {
  event.preventDefault();
  reRenderPage({ page });
};


// UI presentation elements
const datatable = () => {
  const blockHash = $('#block-hash').text();

  $('#txs-table').DataTable({
    ...window.datatable.baseConfig,
    ajax: `/api/block/${blockHash}/transactions`,
    columns: [
      { data: 'txHash', title: 'ID', className: 'hash', render: window.datatable.renderTxHash },
      { data: 'size', title: 'Size', render: window.datatable.renderSize },
      { name: 'fee', title: 'Fee [sats]', className: 'fee', render: window.datatable.renderFee },
      { data: 'numInputs', title: 'Inputs' },
      { data: 'numOutputs', title: 'Outputs' },
      { data: 'satsOutput', title: 'Output Amount', render: window.datatable.renderOutput },
      { name: 'responsive', render: () => '' },
    ]
  });

  params = window.state.getParameters();
  $('#txs-table').dataTable().api().page.len(params.rows);
};


// events
$(window).resize(() => {
  const { currentPage, pageArray } = window.pagination.generatePaginationUIParams();
  window.pagination.generatePaginationUI(currentPage, pageArray);
  $('#blocks-table').DataTable().responsive.rebuild();
  $('#blocks-table').DataTable().responsive.recalc();
});

$('#txs-table').on('init.dt', () => {
  $('.datatable__length-placeholder').remove();
} );

$('#txs-table').on('length.dt', (e, settings, rows) => {
  params = window.state.getParameters();

  if (params.rows !== rows) {
    reRenderPage({ rows });
  }
} );

$('#txs-table').on('xhr.dt', () => {
  updateLoading(false);
} );


// Basically a fake refresh, dynamically updates everything
// according to new params
// updates: URL, table and pagination
const reRenderPage = params => {
  if (params) {
    window.state.updateParameters(params)

    if (params.page) {
      $('#txs-table').DataTable().page(params.page).draw(false);
    }
  }

  const { currentPage, pageArray } = window.pagination.generatePaginationUIParams();
  window.pagination.generatePaginationUI(currentPage, pageArray);
};


// main
$(document).ready(() => {
  // init all UI elements
  datatable()

  // global state update
  reRenderPage();
});
