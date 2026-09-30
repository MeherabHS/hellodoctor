const fs = require('fs');
const html = fs.readFileSync('prototype/index.html', 'utf8');

const endScriptIdx = html.lastIndexOf('</script>');
console.log(html.substring(endScriptIdx - 1200, endScriptIdx));
