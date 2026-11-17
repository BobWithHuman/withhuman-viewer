// A step's text as it was typed: a shell command after a `$`, laid out as a script (a statement a
// line, a loop's body indented, a long pipe cut); arguments in JSON laid out; a path breaking
// only where it divides, never inside a name.

/** One line of a command laid out as a script: how deep in a loop or branch it sits, a pipe it
 * goes on with, and the `&&` or `||` it ends with. */
export type Line = { level: number; text: string; lead?: string; end?: string };

/** A command cut at the `;`, new lines, `&&` and `||` that chain it (or at `|`), outside quotes,
 * `$( )` and `{ }`; each part with what followed it. */
function cut(cmd: string, ops: string[]): { text: string; end: string }[] {
  const out: { text: string; end: string }[] = [];
  let cur = "", quote = "", depth = 0;
  for (let i = 0; i < cmd.length; i++) {
    const c = cmd[i];
    if (c === "\\" && quote !== "'") { cur += c + (cmd[i + 1] ?? ""); i++; continue; }
    if (quote) { if (c === quote) quote = ""; cur += c; continue; }
    if (c === "'" || c === '"') { quote = c; cur += c; continue; }
    if (c === "(" || c === "{") depth++;
    if (c === ")" || c === "}") depth = Math.max(0, depth - 1);
    const hit = depth === 0 && ops.find((o) => cmd.startsWith(o, i) && !(o === "|" && (cmd[i + 1] === "|" || cmd[i - 1] === "|")));
    if (hit) {
      out.push({ text: cur.trim(), end: hit });
      cur = "";
      i += hit.length - 1;
      continue;
    }
    cur += c;
  }
  out.push({ text: cur.trim(), end: "" });
  return out;
}

const LONG = 72;

/** A command as a script would be written: a statement a line, the body of a loop or branch
 * indented under its `do` or `then`, `&&` and `||` at the end of the line they follow, and a line
 * too long cut at its pipes. */
export function script(cmd: string): Line[] {
  const lines: Line[] = [];
  let level = 0;
  for (const { text, end } of cut(cmd, ["&&", "||", ";", "\n"])) {
    let t = text;
    const op = end === "&&" || end === "||" ? end : undefined;
    for (let guard = 0; t && guard < 8; guard++) {
      const kw = /^(do|then|else|elif|done|fi|esac)\b\s*/.exec(t);
      if (!kw) break;
      const k = kw[1];
      if (k === "do" || k === "then") {
        const prev = lines[lines.length - 1];
        if (prev) prev.text += `; ${k}`; else lines.push({ level, text: k });
        level++;
        t = t.slice(kw[0].length);
      } else if (k === "else" || k === "elif") {
        level = Math.max(0, level - 1);
        if (k === "else") { lines.push({ level, text: "else" }); level++; t = t.slice(kw[0].length); }
        else { lines.push({ level, text: t, end: op }); t = ""; }
      } else {
        level = Math.max(0, level - 1);
        lines.push({ level, text: t, end: op });
        t = "";
      }
    }
    if (t) lines.push({ level, text: t, end: op });
  }
  // A line too long goes on at its pipes, one deeper.
  return lines.flatMap((l) => {
    if (l.text.length <= LONG) return [l];
    const parts = cut(l.text, ["|"]);
    if (parts.length < 2) return [l];
    return parts.map((p, i) => ({ level: l.level + (i ? 1 : 0), text: p.text, lead: i ? "|" : undefined, end: i === parts.length - 1 ? l.end : undefined }));
  });
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
  // A here-document keeps its lines as written; any other command is laid out as a script.
  if (isShell(tool)) {
    const raw = text.includes("<<");
    const lines: Line[] = raw ? text.replace(/\s+$/, "").replace(/^\s*\n/, "").split("\n").map((l) => ({ level: 0, text: l })) : script(text);
    return (
      <pre className={box}>
        {lines.map((l, i) => (
          <div key={i} className="flex min-h-[1.6em]">
            <span className="w-[2ch] shrink-0 text-dim select-none">{i ? (raw ? ">" : "") : "$"}</span>
            <span className="min-w-0 flex-1" style={{ paddingLeft: `${l.level * 2 + 2}ch`, textIndent: "-2ch" }}>
              {l.lead && <span className="font-semibold text-mut">{l.lead} </span>}
              <Marked text={l.text} mark={mark} first={!l.lead} />
              {l.end && <span className="font-semibold text-mut"> {l.end}</span>}
            </span>
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
