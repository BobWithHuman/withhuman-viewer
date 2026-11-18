// What a shell command does, in a few words: each statement of it by its program, as a verb and
// what it acts on (`csv_export.py 수정`, `테스트 실행`, `api.github.com 요청`); `cd`, `echo` and
// the like, which do nothing worth saying, left out.

import { script } from "./code";

/** A statement's words as the shell splits them, quotes taken off. */
function words(cmd: string): string[] {
  const out: string[] = [];
  let cur = "", quote = "", any = false;
  for (let i = 0; i < cmd.length; i++) {
    const c = cmd[i];
    if (quote) { if (c === quote) quote = ""; else cur += c; continue; }
    if (c === "'" || c === '"') { quote = c; any = true; continue; }
    if (/\s/.test(c)) { if (cur || any) out.push(cur); cur = ""; any = false; continue; }
    cur += c;
  }
  if (cur || any) out.push(cur);
  return out;
}

const base = (p: string) => p.replace(/[\\/]+$/, "").split(/[\\/]/).pop() || p;
const files = (args: string[]) => args.filter((a) => !a.startsWith("-") && /[./\\]/.test(a) && !/^\d/.test(a));
const host = (args: string[]) => { for (const a of args) { const m = /^(?:https?:\/\/)?([a-z0-9-]+(?:\.[a-z0-9-]+)+)(?::\d+)?(?:\/|$)/i.exec(a); if (m && !a.startsWith("-")) return m[1]; } return null; };
const SILENT = new Set(["cd", "pushd", "popd", "echo", "printf", "pwd", "set", "export", "true", "false", "sleep", "clear", "Set-Location", "Write-Host", "Write-Output"]);

/** One statement, in a few words; null when it does nothing worth saying. */
function statement(text: string): string | null {
  // A pipe's first program does the work; what follows shapes its output.
  const first = text.split(/\s\|\s/)[0];
  let w = words(first).filter((x) => !/^\w+=/.test(x));
  while (w[0] && ["sudo", "env", "time", "nohup", "exec"].includes(w[0])) w = w.slice(1);
  // A loop's or a branch's own words (`for f in *.bin; do`, `done`, `fi`) do nothing themselves.
  if (/^(for|while|until|if|elif|else|then|do|done|fi|case|esac|function|\{|\})$/.test(w[0] ?? "")) return null;
  const [prog0, ...args] = w;
  if (!prog0) return null;
  const prog = base(prog0).replace(/\.exe$/i, "");
  if (SILENT.has(prog)) return null;
  const f = files(args), one = f[f.length - 1] && base(f[f.length - 1]);
  const sub = args.find((a) => !a.startsWith("-"));
  switch (prog) {
    case "sed": case "perl": return args.some((a) => /^-i/.test(a)) ? `${one ?? "파일"} 수정` : `${one ?? "글"} 걸러 보기`;
    case "cat": case "head": case "tail": case "less": case "more": case "type": case "Get-Content": case "bat": case "xxd": case "od": return `${one ?? "파일"} 읽기`;
    case "ls": case "dir": case "tree": case "Get-ChildItem": case "find": return `${one ?? "폴더"} 목록 보기`;
    case "grep": case "rg": case "egrep": case "findstr": case "Select-String": return `${sub ? `"${sub}"` : "글"} 찾기`;
    case "rm": case "del": case "rmdir": case "Remove-Item": return `${one ?? sub ?? "파일"} 삭제`;
    case "mv": case "move": case "Move-Item": return `${one ?? "파일"} 옮기기`;
    case "cp": case "copy": case "Copy-Item": return `${one ?? "파일"} 복사`;
    case "mkdir": case "New-Item": return `${one ?? sub ?? "폴더"} 만들기`;
    case "touch": return `${one ?? "파일"} 만들기`;
    case "chmod": case "chown": return `${one ?? "파일"} 권한 바꾸기`;
    case "curl": case "wget": case "Invoke-WebRequest": case "iwr": case "http": return `${host(args) ?? "웹"} 요청`;
    case "scp": case "rsync": case "ssh": return `${host(args.map((a) => a.replace(/^[^@]+@/, "").replace(/:.*$/, ""))) ?? "다른 기기"} 연결`;
    case "pytest": return "테스트 실행";
    case "python": case "python3": case "py": case "node": case "deno": case "bun": case "ruby": case "bash": case "sh": case "zsh": case "pwsh": case "powershell": {
      const m = args.indexOf("-m");
      if (m >= 0 && args[m + 1]) return /pytest|unittest/.test(args[m + 1]) ? "테스트 실행" : args[m + 1] === "pip" ? `${args.slice(m + 3).filter((a) => !a.startsWith("-")).join(", ") || "패키지"} 설치` : `${args[m + 1]} 실행`;
      if (args.includes("-c") || args.includes("-e")) return `${prog} 코드 실행`;
      return one ? `${one} 실행` : `${prog} 실행`;
    }
    case "pip": case "pip3": case "npm": case "pnpm": case "yarn": case "cargo": case "go": case "brew": case "apt": case "apt-get": {
      if (sub === "install" || sub === "add" || sub === "i") return `${args.filter((a) => !a.startsWith("-")).slice(1).join(", ") || "패키지"} 설치`;
      if (sub === "test" || (sub === "run" && /test/.test(args[1] ?? ""))) return "테스트 실행";
      if (sub === "run") return `${args[1] ?? ""} 실행`.trim();
      return `${prog} ${sub ?? ""}`.trim();
    }
    case "git": case "gh": case "docker": case "kubectl": case "terraform": case "aws": case "gcloud": case "az": return `${prog} ${sub ?? ""}`.trim();
    default: return one ? `${prog} ${one}` : prog;
  }
}

/** A command in a few words: its statements' doings, each once, the first few. */
export function doing(cmd: string): string | null {
  const said = script(cmd).filter((l) => !l.lead).map((l) => statement(l.text)).filter((x): x is string => !!x);
  const unique = [...new Set(said)];
  if (!unique.length) return null;
  return unique.slice(0, 3).join(" · ") + (unique.length > 3 ? ` 외 ${unique.length - 3}` : "");
}
