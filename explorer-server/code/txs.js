const renderHash = hash => '<a href="/tx/' + hash + '">' + hash + '</a>';
const renderSize = size => formatByteSize(size);
const renderFee = (_value, _type, row) => {
  console.log(row)
  if (row.isCoinbase) {
    return '<div class="ui green horizontal label">Coinbase</div>';
  }

  const fee = renderInteger(row.stats.satsInput - row.stats.satsOutput);
  let markup = '';

  markup += `<span>${fee}</span>`
  markup += `<span class="fee-per-byte">(${renderFeePerByte(_value, _type, row)})</span>`

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
const renderOutput = (satsOutput, _type, row) => {
  if (row.token) {
    var ticker = ' <a href="/tx/' + row.token.tokenId + '">' + row.token.tokenTicker + '</a>';
    return renderAmount(row.stats.tokenOutput, row.token.decimals) + ticker;
  }
  return renderSats(row.stats.satsOutput) + ' XEC';
};


const updateLoading = (status) => {
  if (status) {
    $('#txs-table > tbody').addClass('blur');
    $('.loader__container--fullpage').removeClass('hidden');
    $('#pagination').addClass('hidden');
    $('#footer').addClass('hidden');
  } else {
    $('#txs-table > tbody').removeClass('blur');
    $('.loader__container--fullpage').addClass('hidden');
    $('#pagination').removeClass('hidden');
    $('#footer').removeClass('hidden');
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
    searching: false,
    lengthMenu: [50, 100, 250, 500, 1000],
    pageLength: DEFAULT_ROWS_PER_PAGE,
    language: {
      loadingRecords: '',
      zeroRecords: '',
      emptyTable: '',
    },
    ajax: `/api/block/${blockHash}/transactions`,
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
    columns: [
      { data: 'txHash', title: 'ID', className: 'hash', render: renderHash },
      { data: 'size', title: 'Size', render: renderSize },
      { name: 'fee', title: 'Fee [sats]', css: 'fee', render: renderFee },
      { data: 'numInputs', title: 'Inputs' },
      { data: 'numOutputs', title: 'Outputs' },
      { data: 'satsOutput', title: 'Output Amount', render: renderOutput },
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
webix.ready(() => {
  // init all UI elements
  datatable()

  // global state update
  reRenderPage();
});
