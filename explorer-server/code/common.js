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

function renderTxHash(txHash) {
  return txHash.substr(0, 10) + '&hellip;' + txHash.substr(60, 4)
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
    $(this).toggleClass('hidden');
  });
}

function minifyHash(hash) {
  return `${hash.slice(0, 25)}...${hash.slice(39, 64)}`;
}
