// A step's text as it was typed: a shell command after a `$`, each chained part on its own line;
// arguments in JSON laid out; a path breaking only where it divides, never inside a name.

const OPS = ["&&", "||", "|", ";"];

/** A command cut at the `&&`, `||`, `|` and `;` that chain it, outside quotes and `$( )`. */
export function chain(cmd: string): { op: string; part: string }[] {
  const out: { op: string; part: string }[] = [];
  let cur = "", op = "", quote = "", depth = 0;
  for (let i = 0; i < cmd.length; i++) {
    const c = cmd[i];
    if (c === "\\" && quote !== "'") { cur += c + (cmd[i + 1] ?? ""); i++; continue; }
    if (quote) { if (c === quote) quote = ""; cur += c; continue; }
    if (c === "'" || c === '"') { quote = c; cur += c; continue; }
    if (c === "(") depth++;
    if (c === ")") depth = Math.max(0, depth - 1);
    const hit = depth === 0 && OPS.find((o) => cmd.startsWith(o, i));
    if (hit) {
      out.push({ op, part: cur.trim() });
      cur = "";
      op = hit;
      i += hit.length - 1;
      continue;
    }
    cur += c;
  }
  out.push({ op, part: cur.trim() });
  return out.filter((x) => x.part);
}

const isShell = (tool?: string) => !!tool && /^(Bash|bash|sh|zsh|shell|PowerShell|exec_command)$/i.test(tool);

/** JSON laid out, when the text is JSON. */
function laidOut(text: string): string | null {
  const t = text.trim();
  if (!/^[[{]/.test(t)) return null;
  try { return JSON.stringify(JSON.parse(t), null, 2); } catch { return null; }
}

/** A word that breaks after a `/` or `\`; each piece between kept whole while it fits the line,
 * and broken anywhere when it is longer than the line. */
function Word({ w }: { w: string }) {
  const parts = w.split(/(?<=[/\\])/);
  return <>{parts.map((p, i) => <span key={i} className="inline-block max-w-full align-top [overflow-wrap:anywhere] indent-0">{p}</span>)}</>;
}

/** Words of a line kept whole, the spaces and tabs between them as they were. */
function Words({ text, first }: { text: string; first?: boolean }) {
  let seen = false;
  return (
    <>
      {text.split(/(\s+)/).map((w, i) => {
        if (!w || /^\s+$/.test(w)) return w;
        const prog = first && !seen;
        seen = true;
        const cls = prog ? "font-semibold text-white" : /^--?[\w-]/.test(w) ? "text-[#8fb3e0]" : /^['"]/.test(w) ? "text-[#d8c38e]" : undefined;
        return <span key={i} className={cls}><Word w={w} /></span>;
      })}
    </>
  );
}

/** One line with the part to show as the problem marked. */
function Marked({ text, mark, first }: { text: string; mark?: string; first?: boolean }) {
  if (!mark || !text.includes(mark)) return <Words text={text} first={first} />;
  const parts = text.split(mark);
  return (
    <>
      {parts.map((p, i) => (
        <span key={i}><Words text={p} first={first && i === 0} />{i < parts.length - 1 && <mark className="rounded-[3px] bg-block/25 px-0.5 text-[#ffb4ab]"><Words text={mark} /></mark>}</span>
      ))}
    </>
  );
}

export function Code({ text, mark, tool }: { text: string; mark?: string; tool?: string }) {
  const box = "m-0 rounded-md bg-box px-3 py-2.5 font-mono text-[11.5px] leading-[1.6] whitespace-pre-wrap text-[#e8e8e8] [overflow-wrap:anywhere] [tab-size:4]";
  // A script of many lines as a terminal shows one typed: `$` and then `>`, each line as written.
  if (isShell(tool) && text.trim().includes("\n")) {
    const lines = text.replace(/\s+$/, "").replace(/^\s*\n/, "").split("\n");
    return (
      <pre className={box}>
        {lines.map((l, i) => (
          <div key={i} className="min-h-[1.6em] -indent-[2ch] pl-[2ch]">
            <span className="text-dim select-none">{i ? "> " : "$ "}</span><Marked text={l} mark={mark} first />
          </div>
        ))}
      </pre>
    );
  }
  if (isShell(tool)) {
    const parts = chain(text);
    const split = parts.length > 1 && text.length > 48;
    const lines = split ? parts : [{ op: "", part: text.trim() }];
    return (
      <pre className={box}>
        {lines.map((l, i) => (
          <div key={i} className={i ? "-indent-[2ch] pl-[4ch]" : "-indent-[2ch] pl-[2ch]"}>
            {i === 0 ? <span className="text-dim select-none">$ </span> : <span className="font-semibold text-mut">{l.op} </span>}
            <Marked text={l.part} mark={mark} first />
          </div>
        ))}
      </pre>
    );
  }
  const body = laidOut(text) ?? text;
  return (
    <pre className={box}>
      {body.split("\n").map((l, i) => <div key={i} className="min-h-[1.6em]"><Marked text={l} mark={mark} /></div>)}
    </pre>
  );
}
