// 打印前的文本整理（像 lint 一样：规则可开关，返回修了几处）

export type Rules = {
  whitespace: boolean;
  fillers: boolean;
  pangu: boolean;
  punctuation: boolean;
  /** 每行一条 "原文 => 替换后"；原文写成 /pattern/flags 时按正则处理 */
  custom: string;
};

const CJK = "\\u3040-\\u30ff\\u3400-\\u4dbf\\u4e00-\\u9fff\\uf900-\\ufaff";
const BOUND = `(^|[\\s，。！？、,.!?；;：:])`;

const FILLERS = [
  // 中文
  "嗯+", "呃+", "额+", "唔+", "那个那个", "就是说",
  // 日本語
  "え[ーっ]+と", "えー+", "あのー+", "うーん", "まあ",
  // English
  "um+", "uh+", "erm", "you know",
];

type Step = { re: RegExp; to: string | ((...m: string[]) => string) };

function customSteps(src: string): { steps: Step[]; bad: string[] } {
  const steps: Step[] = [];
  const bad: string[] = [];
  for (const raw of src.split("\n")) {
    const line = raw.trim();
    if (!line || line.startsWith("#")) continue;
    const i = line.indexOf("=>");
    if (i < 0) {
      bad.push(line);
      continue;
    }
    const from = line.slice(0, i).trim();
    const to = line.slice(i + 2).trim();
    try {
      const m = from.match(/^\/(.+)\/([gimsuy]*)$/);
      const re = m
        ? new RegExp(m[1], m[2].includes("g") ? m[2] : m[2] + "g")
        : new RegExp(from.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"), "g");
      steps.push({ re, to });
    } catch {
      bad.push(line);
    }
  }
  return { steps, bad };
}

/** 返回整理后的文本、修改次数，以及写错的自定义规则 */
export function lint(input: string, rules: Rules): { text: string; fixes: number; badRules: string[] } {
  let text = input;
  let fixes = 0;
  const run = (re: RegExp, to: Step["to"]) => {
    text = text.replace(re, (...m: string[]) => {
      const out = typeof to === "string" ? to.replace(/\$(\d)/g, (_, i) => m[Number(i)] ?? "") : to(...m);
      if (out !== m[0]) fixes++;
      return out;
    });
  };

  // 自定义替换最先跑，替换出来的文字再交给后面的规则整理
  const { steps, bad } = customSteps(rules.custom);
  for (const s of steps) run(s.re, s.to);

  if (rules.fillers) {
    const alt = FILLERS.join("|");
    run(new RegExp(`${BOUND}(?:${alt})(?![A-Za-z])[\\s，、,…]*`, "gi"), (_m, b) => b);
  }
  if (rules.whitespace) {
    run(new RegExp(`([${CJK}])[ \\t]+(?=[${CJK}])`, "g"), "$1");
    run(/[ \t]{2,}/g, " ");
    run(/^[ \t]+|[ \t]+$/gm, "");
  }
  if (rules.punctuation) {
    const map: Record<string, string> = { ",": "，", "?": "？", "!": "！", ":": "：", ";": "；" };
    run(new RegExp(`([${CJK}])\\s*([,?!:;])(?!\\d)`, "g"), (_m, c, p) => c + map[p]);
    run(new RegExp(`([${CJK}])\\s*\\.(?=\\s|$|[${CJK}])`, "g"), "$1。");
  }
  if (rules.pangu) {
    run(new RegExp(`([${CJK}])([A-Za-z0-9@#&%$])`, "g"), "$1 $2");
    run(new RegExp(`([A-Za-z0-9%$])([${CJK}])`, "g"), "$1 $2");
  }
  return { text: text.trim(), fixes, badRules: bad };
}
