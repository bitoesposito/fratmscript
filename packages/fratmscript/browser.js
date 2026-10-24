// Runs <script type="text/fratmscript"> tags in a web page:
//
//   <script type="module" src="https://cdn.jsdelivr.net/npm/fratmscript/browser.js"></script>
//   <script type="text/fratmscript">
//     stamm a dì("Uè!")
//   </script>
//
// Scripts run in document order once the page has loaded, inline or with
// src="file.fratm". Compile errors and uncaught runtime errors are printed in
// Neapolitan in the console. `ready` resolves when every script has run.
import { init, compile, runtimeError } from './web.js';
import { runScripts } from './lib/scripts.js';

function execute(code, original, isModule) {
  const script = document.createElement('script');
  if (isModule) script.type = 'module';
  script.textContent = code;
  // Insert next to the original tag and drop it again: the code has run
  // (classic scripts run synchronously when inserted).
  original.after(script);
  script.remove();
}

async function load(url) {
  const response = await fetch(url);
  if (!response.ok) throw new Error(`${response.status} ${response.statusText}`);
  return response.text();
}

function domReady() {
  if (document.readyState !== 'loading') return Promise.resolve();
  return new Promise((resolve) => document.addEventListener('DOMContentLoaded', resolve, { once: true }));
}

window.addEventListener('error', (event) => {
  if (event.error instanceof Error) console.error(`🤌 ${runtimeError(event.error)}`);
});
window.addEventListener('unhandledrejection', (event) => {
  if (event.reason instanceof Error) console.error(`🤌 ${runtimeError(event.reason)}`);
});

export const ready = Promise.all([init(), domReady()]).then(() =>
  runScripts({ document, load, compile, execute, report: (message) => console.error(message) }),
);
