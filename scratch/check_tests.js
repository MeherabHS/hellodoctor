const fs = require('fs');

const files = ['scratch/test_admin_suite.js', 'scratch/test_grievance_suite.js', 'scratch/test_admin_dom.js', 'scratch/test_admin_logs.js', 'scratch/test_admin_searchbars.js', 'scratch/test_admin_runtime_sim.js'];

files.forEach(f => {
  if (fs.existsSync(f)) {
    const c = fs.readFileSync(f, 'utf8');
    console.log(`=== ${f} ===`);
    const lines = c.split('\n').filter(l => l.includes('adminNav-') || l.includes('adminPage_'));
    console.log(lines.join('\n'));
  }
});
