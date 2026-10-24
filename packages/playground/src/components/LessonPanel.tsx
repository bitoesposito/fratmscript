import { Button } from '@/components/ui/button'
import { NativeSelect } from '@/components/ui/select'
import { cn } from '@/lib/utils'
import type { Lesson } from '@/lib/lessons'
import { ChevronLeft, ChevronRight, CircleCheck, Lightbulb, RotateCcw, X } from 'lucide-react'
import { Fragment } from 'react'

interface LessonPanelProps {
  lessons: Lesson[]
  index: number
  /** Ids of the lessons whose exercise is done */
  completed: Set<string>
  onSelect: (index: number) => void
  onExit: () => void
  onReset: () => void
  onShowSolution: () => void
}

/** Text with `code` spans (and ``a ` inside`` for backticks). */
function RichText({ text }: { text: string }) {
  const parts = text.split(/(``\s?.+?\s?``|`[^`]+`)/g)
  return (
    <>
      {parts.map((part, i) => {
        if (part.startsWith('`') && part.endsWith('`') && part.length > 1) {
          const code = part.startsWith('``') ? part.slice(2, -2).trim() : part.slice(1, -1)
          return (
            <code key={i} className="bg-muted px-1 py-0.5 rounded font-mono text-[0.85em] text-foreground">
              {code}
            </code>
          )
        }
        return <Fragment key={i}>{part}</Fragment>
      })}
    </>
  )
}

export function LessonPanel({ lessons, index, completed, onSelect, onExit, onReset, onShowSolution }: LessonPanelProps) {
  const lesson = lessons[index]
  const done = completed.has(lesson.id)
  const isLast = index === lessons.length - 1

  return (
    <section aria-label="Lezione" className="flex flex-col bg-card border-border border-b max-h-[60%] md:max-h-[55%] shrink-0">
      <div className="flex justify-between items-center gap-2 px-4 pt-3 shrink-0">
        <NativeSelect
          aria-label="Indice d'e lezioni"
          value={String(index)}
          onChange={e => onSelect(Number(e.target.value))}
          className="w-60 max-w-[80%]"
        >
          {lessons.map((l, i) => (
            <option key={l.id} value={i}>
              {completed.has(l.id) ? '✓ ' : ''}{i + 1}. {l.title}
            </option>
          ))}
        </NativeSelect>
        <Button variant="ghost" size="icon-sm" aria-label="Esci d'a scola" onClick={onExit}>
          <X className="size-4" />
        </Button>
      </div>

      <div className="flex-1 space-y-2 px-4 py-3 min-h-0 overflow-auto text-sm leading-relaxed">
        <h2 className="font-semibold text-base text-foreground">
          <span className="mr-1.5 text-muted-foreground">{index + 1}/{lessons.length}</span>
          {lesson.title}
        </h2>
        {lesson.body.map((paragraph, i) => (
          <p key={i} className="text-muted-foreground"><RichText text={paragraph} /></p>
        ))}
      </div>

      <div
        className={cn(
          'mx-4 mb-3 p-3 border rounded-md text-sm shrink-0',
          done ? 'border-green-600/40 bg-green-600/10' : 'border-primary/30 bg-primary/5',
        )}
        aria-live="polite"
      >
        <p className="font-medium text-foreground">
          {done ? (
            <span className="inline-flex items-center gap-1.5 text-green-700 dark:text-green-400">
              <CircleCheck className="size-4" /> Bravo! 🤌
            </span>
          ) : (
            'Esercizio'
          )}
        </p>
        <p className="mt-1 text-muted-foreground"><RichText text={lesson.task} /></p>
        {!done && (
          <p className="mt-1 text-muted-foreground text-xs">
            'A console adda stampà: <code className="font-mono">{lesson.expected.join(' ⏎ ')}</code>
          </p>
        )}
      </div>

      <div className="flex justify-between items-center gap-2 px-4 pb-3 shrink-0">
        <div className="flex gap-1">
          <Button variant="ghost" size="sm" onClick={onReset} title="Torna al codice iniziale" aria-label="Ricomincia">
            <RotateCcw className="size-3.5" /> <span className="hidden sm:inline">Ricomincia</span>
          </Button>
          <Button variant="ghost" size="sm" onClick={onShowSolution} title="Mostra la soluzione" aria-label="Soluzione">
            <Lightbulb className="size-3.5" /> <span className="hidden sm:inline">Soluzione</span>
          </Button>
        </div>
        <div className="flex gap-1">
          <Button variant="outline" size="sm" disabled={index === 0} onClick={() => onSelect(index - 1)} aria-label="Lezione precedente">
            <ChevronLeft className="size-4" />
          </Button>
          {isLast ? (
            <Button size="sm" onClick={onExit}>Fernuto</Button>
          ) : (
            <Button size="sm" variant={done ? 'default' : 'outline'} onClick={() => onSelect(index + 1)}>
              Appriesso <ChevronRight className="size-4" />
            </Button>
          )}
        </div>
      </div>
    </section>
  )
}
