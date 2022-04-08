const updateLoading = (status) => {
  if (status) {
    $('#blocks-table > tbody').addClass('blur');
    $('.loader__container--fullpage').removeClass('visibility-hidden');
    $('#pagination').addClass('visibility-hidden');
    $('#footer').addClass('visibility-hidden');
  } else {
    $('#blocks-table > tbody').removeClass('blur');
    $('.loader__container--fullpage').addClass('visibility-hidden');
    $('#pagination').removeClass('visibility-hidden');
    $('#footer').removeClass('visibility-hidden');
  }
};


// data fetching
const updateTable = (startPosition, endPosition) => {
  updateLoading(true);
  $('#blocks-table').dataTable().api().ajax.url(`/api/blocks/${endPosition}/${startPosition}`).load()
}


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

  $('#date').text(`Date (${tzString})`)

  $('#blocks-table').DataTable({
    ...window.datatable.baseConfig,
    columns: [
      { name: 'age', data: 'timestamp', orderable: false, render: window.datatable.renderAge },
      { data: 'height', render: window.datatable.renderTemplate },
      { data: 'numTxs', render: window.datatable.renderNumtTxs },
      { data: 'hash', orderable: false, className: 'hash', render: window.datatable.renderBlockHash },
      { data: 'size', orderable: false, render: window.datatable.renderSize },
      { data: 'difficulty', orderable: false, render: window.datatable.renderDifficulty },
      { name: 'timestamp', data: 'timestamp', render: window.datatable.renderTimestamp },
      { name: 'responsive', render: () => '' },
    ]
  });

  params = window.state.getParameters();
  $('#blocks-table').dataTable().api().page.len(params.rows);
}

// events
$(window).resize(() => {
  const { currentPage, pageArray } = window.pagination.generatePaginationUIParams();
  window.pagination.generatePaginationUI(currentPage, pageArray);
  $('#blocks-table').DataTable().responsive.rebuild();
  $('#blocks-table').DataTable().responsive.recalc();
});

// datatable events
$('#blocks-table').on('init.dt', () => {
  $('.datatable__length-placeholder').remove();
} );

$('#blocks-table').on('length.dt', (e, settings, rows) => {
  params = window.state.getParameters();

  if (params.rows !== rows) {
    reRenderPage({ rows });
  }
} );

$('#blocks-table').on('xhr.dt', () => {
  updateLoading(false);
} );

// Basically a fake refresh, dynamically updates everything
// according to new params
// updates: URL, table and pagination
const reRenderPage = params => {
  if (params) {
    window.state.updateParameters(params)
  }

  const [ startPosition, endPosition ] = window.pagination.generatePaginationRequestRange();
  updateTable(startPosition, endPosition);

  const { currentPage, pageArray } = window.pagination.generatePaginationUIParams();
  window.pagination.generatePaginationUI(currentPage, pageArray);
};

// main
$(document).ready(() => {
  // init all UI elements
  dataTable();

  // global state update
  reRenderPage();
});
