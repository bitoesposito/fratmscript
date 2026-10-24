// The example picker lists the programs in the repo's examples/ folder, so the
// playground always shows the same code that CI compiles and golden-tests.

const sources = import.meta.glob('../../../../examples/*.fratm', {
  query: '?raw',
  import: 'default',
  eager: true,
}) as Record<string, string>

/**
 * Examples that cannot run in the browser: 09 imports Node modules
 * (node:path, node:os) and 13 reads the terminal with readline.
 */
const NODE_ONLY = new Set(['09_moduli', '13_pomodor_e_muzzarel'])

export interface Example {
  /** File name without extension, e.g. `01_salutatore` */
  id: string
  /** Picker label, from the file's title comment */
  title: string
  code: string
}

/** Title from the header comment (`// Example 1: Neapolitan Greeter`). */
function exampleTitle(id: string, code: string): string {
  const number = id.split('_')[0]
  const header = code
    .split('\n')
    .map(line => line.replace(/^\/\/\s*/, '').trim())
    .find(line => /^Example \d+:/.test(line))
  const name = header ? header.replace(/^Example \d+:\s*/, '') : id.slice(number.length + 1).replace(/_/g, ' ')
  return `${number} · ${name}`
}

export const exampleList: Example[] = Object.entries(sources)
  .map(([path, code]) => {
    const id = path.split('/').pop()!.replace(/\.fratm$/, '')
    return { id, title: exampleTitle(id, code), code }
  })
  .filter(example => !NODE_ONLY.has(example.id))
  .sort((a, b) => a.id.localeCompare(b.id))

export const examples: Record<string, string> = Object.fromEntries(
  exampleList.map(example => [example.id, example.code]),
)

export const defaultCode = `// Try FratmScript!

chist è nome = "Gennaro"

facc saluta(chi) {
    si (chi == nisciun) {
        piglie "E chi si tu?"
    }
    piglie "Ue " + chi + ", comme staje?"
}

stamm a dì(saluta(nome))
stamm a dì(saluta("Ciro"))
stamm a dì(saluta(nisciun))`
