import assert from 'node:assert/strict';
import { test } from 'node:test';
import { compile } from '../index.js';
import { runScripts, formatError, isModule, scriptName } from '../lib/scripts.js';

// Just enough of a DOM for runScripts
function fakeScript({ src = null, text = '' } = {}) {
  return {
    textContent: text,
    getAttribute: (name) => (name === 'src' ? src : null),
  };
}

function fakeDocument(scripts) {
  return {
    baseURI: 'https://example.com/pagina/',
    querySelectorAll: (selector) => {
      assert.equal(selector, 'script[type="text/fratmscript"]');
      return scripts;
    },
  };
}

test('runs inline and external scripts in document order', async () => {
  const scripts = [
    fakeScript({ text: 'chist è x = 1\nstamm a dì(x)' }),
    fakeScript({ src: 'lib/saluto.fratm' }),
    fakeScript({ text: 'stamm a dì(3)' }),
  ];
  const loaded = [];
  const executed = [];
  const ran = await runScripts({
    document: fakeDocument(scripts),
    // The external script answers last, but still runs second
    load: async (url) => {
      loaded.push(url);
      await new Promise((r) => setTimeout(r, 10));
      return 'stamm a dì(2)';
    },
    compile,
    execute: (code, script, module) => executed.push({ code, script, module }),
    report: (message) => assert.fail(message),
  });
  assert.equal(ran, 3);
  assert.deepEqual(loaded, ['https://example.com/pagina/lib/saluto.fratm']);
  assert.deepEqual(
    executed.map((e) => e.code.split('\n')[0]),
    ['const x = 1;', 'console.log(2);', 'console.log(3);'],
  );
  assert.equal(executed[1].script, scripts[1]);
  assert.match(executed[0].code, /\/\/# sourceMappingURL=data:application\/json;charset=utf-8;base64,/);
  assert.match(executed[1].code, /\/\/# sourceURL=saluto\.fratm$/);
  assert.match(executed[2].code, /\/\/# sourceURL=fratmscript-3\.fratm$/);
});

test('reports compile errors in Neapolitan and keeps going', async () => {
  const reports = [];
  const executed = [];
  await runScripts({
    document: fakeDocument([fakeScript({ text: 'stama a dì(1)' }), fakeScript({ text: 'stamm a dì(2)' })]),
    load: async () => '',
    compile,
    execute: (code) => executed.push(code),
    report: (message) => reports.push(message),
  });
  assert.equal(executed.length, 1);
  assert.equal(reports.length, 1);
  assert.match(reports[0], /^FratmScript fratmscript-1\.fratm:1:7: /);
  assert.match(reports[0], /Forse vulive scrivere 'stamm a dì'/);
});

test('reports scripts that fail to load', async () => {
  const reports = [];
  await runScripts({
    document: fakeDocument([fakeScript({ src: 'manca.fratm' })]),
    load: async () => {
      throw new Error('404 Not Found');
    },
    compile,
    execute: () => assert.fail('nothing to run'),
    report: (message) => reports.push(message),
  });
  assert.deepEqual(reports, ['FratmScript: nun riesco a caricà manca.fratm (404 Not Found)']);
});

test('helpers', () => {
  assert.equal(isModule('import { a } from "./a.js";\n'), true);
  assert.equal(isModule('console.log("import");\n'), false);
  assert.equal(scriptName(fakeScript({ src: '/js/app.fratm?v=2' }), 0), 'app.fratm');
  assert.equal(
    formatError('a.fratm', { message: 'Riga 2, colonna 3: Uè', line: 2, column: 3, suggestion: null }),
    'FratmScript a.fratm:2:3: Uè',
  );
});
