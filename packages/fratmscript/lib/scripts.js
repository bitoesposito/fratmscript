// Finds <script type="text/fratmscript"> tags, compiles them and runs the
// JavaScript, in document order. Kept free of globals so it can be tested
// in Node with a fake document (see test/browser.test.js).

export const SCRIPT_TYPE = 'text/fratmscript';

/**
 * @param {object} env
 * @param {Document} env.document
 * @param {(url: string) => Promise<string>} env.load       fetches a `src=` script
 * @param {(source: string, options: object) => any} env.compile  the fratmscript compile()
 * @param {(code: string, script: Element, isModule: boolean) => void} env.execute  runs compiled JS
 * @param {(message: string) => void} env.report             prints a compile error
 * @returns {Promise<number>} how many scripts ran
 */
export async function runScripts({ document, load, compile, execute, report }) {
  const scripts = Array.from(document.querySelectorAll(`script[type="${SCRIPT_TYPE}"]`));
  // Fetch every external script up front, then run them in order
  const sources = scripts.map((script) => {
    const src = script.getAttribute('src');
    return src ? load(new URL(src, document.baseURI).href) : Promise.resolve(script.textContent ?? '');
  });

  let ran = 0;
  for (let i = 0; i < scripts.length; i++) {
    const script = scripts[i];
    const name = scriptName(script, i);
    let source;
    try {
      source = await sources[i];
    } catch (error) {
      report(`FratmScript: nun riesco a caricà ${name} (${error && error.message ? error.message : error})`);
      continue;
    }
    const result = compile(source, { filename: name });
    if (result.errors.length > 0) {
      for (const error of result.errors) report(formatError(name, error));
      continue;
    }
    let code = result.code;
    if (result.map) {
      code += `\n//# sourceMappingURL=data:application/json;charset=utf-8;base64,${base64(JSON.stringify(result.map))}`;
    }
    code += `\n//# sourceURL=${name}`;
    execute(code, script, isModule(result.code));
    ran++;
  }
  return ran;
}

/** File name shown in errors and dev tools: the `src`, or `fratmscript-N.fratm`. */
export function scriptName(script, index) {
  const src = script.getAttribute('src');
  if (src) return src.split(/[?#]/)[0].split('/').pop() || src;
  return `fratmscript-${index + 1}.fratm`;
}

export function formatError(name, error) {
  const where = error.line ? `${name}:${error.line}:${error.column ?? 1}` : name;
  // The message already starts with "Riga X, colonna Y: "; the location is in `where`
  const message = String(error.message).replace(/^Riga \d+, colonna \d+: /, '');
  return `FratmScript ${where}: ${message}${error.suggestion ? `\n${error.suggestion}` : ''}`;
}

/** Compiled code that imports or exports has to run as a module. */
export function isModule(code) {
  return /^(import|export)\b/m.test(code);
}

function base64(text) {
  const bytes = new TextEncoder().encode(text);
  let binary = '';
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary);
}
