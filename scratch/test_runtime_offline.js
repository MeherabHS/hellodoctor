const fs = require('fs');
const assert = require('assert');
const vm = require('vm');

const context = {
  window: {
    addEventListener: () => {}
  },
  document: {
    body: {
      classList: {
        _classes: new Set(),
        add(c) { this._classes.add(c); },
        remove(c) { this._classes.delete(c); },
        toggle(c, force) {
          if (force === undefined) {
            if (this._classes.has(c)) this._classes.delete(c);
            else this._classes.add(c);
          } else if (force) {
            this._classes.add(c);
          } else {
            this._classes.delete(c);
          }
        },
        has(c) { return this._classes.has(c); }
      }
    },
    getElementById: (id) => {
      return {
        id,
        style: {},
        classList: {
          _classes: new Set(),
          add(c) { this._classes.add(c); },
          remove(c) { this._classes.delete(c); },
          has(c) { return this._classes.has(c); }
        },
        textContent: '',
        innerHTML: '',
        title: '',
        addEventListener: () => {},
        appendChild: () => {}
      };
    },
    addEventListener: () => {},
    querySelectorAll: () => [],
    createElement: (tag) => ({
      tagName: tag,
      className: '',
      style: {},
      classList: { add: () => {}, remove: () => {} },
      innerHTML: '',
      remove: () => {}
    })
  },
  setTimeout: (fn) => fn(),
  clearTimeout: () => {}
};

vm.createContext(context);

const htmlFile = fs.existsSync('index.html') ? 'index.html' : 'prototype/index.html';
const html = fs.readFileSync(htmlFile, 'utf8');
const script1 = html.match(/<script[\s\S]*?<\/script>/gi)[0].replace(/<\/?script[^>]*>/gi, '');

vm.runInContext(script1, context);

console.log('Testing setNetworkOnlineState(false)...');
context.setNetworkOnlineState(false);
assert.strictEqual(context.window.isAppOnline, false, 'window.isAppOnline must be false');
assert.ok(context.document.body.classList.has('is-offline'), 'Body should have is-offline class');

console.log('Testing setNetworkOnlineState(true)...');
context.setNetworkOnlineState(true);
assert.strictEqual(context.window.isAppOnline, true, 'window.isAppOnline must be true');
assert.ok(!context.document.body.classList.has('is-offline'), 'Body should not have is-offline class');

console.log('Testing toggleNetworkSimulation()...');
context.toggleNetworkSimulation();
assert.strictEqual(context.window.isAppOnline, false, 'toggleNetworkSimulation should toggle to false');

context.toggleNetworkSimulation();
assert.strictEqual(context.window.isAppOnline, true, 'toggleNetworkSimulation should toggle back to true');

console.log('✅ All VM runtime tests passed with flying colors!');
