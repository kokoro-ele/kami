import { api } from "./api";
import { parseErr, type ErrInfo } from "./errors";
import type { Lang } from "./i18n";
import { formatDate, formatTime, type AiSettings } from "./settings";
import { Sheet, type PaperFont } from "./sheet";

export type Fortune = {
  luck: string;
  title: string;
  poem: string[];
  advice: string;
  lucky_item: string;
};

type Pack = {
  prompt: (wish: string) => string;
  luck: string[];
  titles: string[];
  poems: string[][];
  advice: string[];
  items: string[];
  card: { head: string; sub: string; wishLabel: string; adviceLabel: string; itemLabel: string; foot: string };
};

const JSON_SPEC = `{"luck":"…","title":"…","poem":["…","…","…","…"],"advice":"…","lucky_item":"…"}`;

const PACKS: Record<Lang, Pack> = {
  zh: {
    prompt: (w) =>
      `你是神社门口的许愿机。为下面的愿望写一张签，温暖、带一点幽默、不敷衍。只输出 JSON：${JSON_SPEC}\n` +
      `luck 取「大吉/中吉/小吉/吉」之一；title 是 4~8 字的签名；poem 是四句签诗，每句不超过 12 字；advice 是一句今天就能做的小建议（30 字内）；lucky_item 是幸运物（8 字内）。\n愿望：${w}`,
    luck: ["大吉", "大吉", "中吉", "中吉", "小吉", "吉"],
    titles: ["心想事成签", "春风得意签", "水到渠成签", "柳暗花明签"],
    poems: [
      ["一念起时风已动", "千山虽远步能通", "今朝落笔成心愿", "他日回看笑春风"],
      ["云开见月本无心", "水到渠成不必寻", "且把今朝当作始", "一步一印是黄金"],
      ["灯火阑珊人未散", "转角便是柳花明", "心中所愿皆可往", "脚下有风自有程"],
    ],
    advice: ["把愿望拆成今天就能做的第一步，然后去做。", "今晚早点睡，明天的你会更有力气。", "给一个很久没联系的人发条消息。"],
    items: ["一杯热茶", "旧车票", "蓝色的笔", "便利店饭团"],
    card: { head: "◇  許 願 機  ◇", sub: "WISH MACHINE", wishLabel: "你的愿望", adviceLabel: "◆ 建议", itemLabel: "◆ 幸运物", foot: "愿望已送达，请收好这张签" },
  },
  ja: {
    prompt: (w) =>
      `あなたは神社のおみくじ機です。次の願いごとに、あたたかく少しユーモアのあるおみくじを書いてください。JSON だけを出力：${JSON_SPEC}\n` +
      `luck は「大吉・中吉・小吉・吉・末吉」のどれか。title は 4〜8 字の題。poem は和歌風の 4 行（各 12 字以内）。advice は今日できる小さな一歩（30 字以内）。lucky_item はラッキーアイテム（8 字以内）。\n願いごと：${w}`,
    luck: ["大吉", "大吉", "中吉", "小吉", "吉", "末吉"],
    titles: ["願望成就", "春風満帆", "雨過天晴", "一陽来復"],
    poems: [
      ["風の音に", "願いをのせて", "歩みゆけば", "道はおのずと"],
      ["雲晴れて", "月はしずかに", "照らすなり", "迷いし道も"],
      ["桜まつ", "枝にちいさき", "つぼみあり", "春はかならず"],
    ],
    advice: ["願いを今日できる一歩に分けて、まず一つやってみましょう。", "今夜は早めに寝ましょう。", "しばらく連絡していない人に一言送ってみましょう。"],
    items: ["温かいお茶", "古い切符", "青いペン", "コンビニのおにぎり"],
    card: { head: "◇  御 神 籤  ◇", sub: "OMIKUJI", wishLabel: "願いごと", adviceLabel: "◆ ひとこと", itemLabel: "◆ ラッキーアイテム", foot: "願いは届きました。大切にお持ちください" },
  },
  en: {
    prompt: (w) =>
      `You are a fortune machine at a shrine gate. Write a warm, slightly funny, sincere fortune for the wish below. Output JSON only: ${JSON_SPEC}\n` +
      `luck is one of "Great Blessing", "Blessing", "Small Blessing", "Good Luck"; title is 2–4 words; poem is four short lines (max 6 words each); advice is one small step for today (max 15 words); lucky_item is under 4 words.\nWish: ${w}`,
    luck: ["Great Blessing", "Blessing", "Blessing", "Small Blessing", "Good Luck"],
    titles: ["Wind at Your Back", "Clear Skies Ahead", "The Long Way Home"],
    poems: [
      ["A wish set on the wind", "travels farther than you think", "start with one small step", "and the road will meet you"],
      ["Clouds part on their own", "the moon was always there", "begin where you stand", "every step is gold"],
    ],
    advice: ["Break the wish into one step you can take today.", "Go to bed early tonight.", "Message someone you haven't talked to in a while."],
    items: ["A warm cup of tea", "An old ticket", "A blue pen", "A rice ball"],
    card: { head: "◇  WISH MACHINE  ◇", sub: "FORTUNE", wishLabel: "Your wish", adviceLabel: "◆ Advice", itemLabel: "◆ Lucky item", foot: "Your wish has been received" },
  },
};

const pick = <T,>(a: T[]) => a[Math.floor(Math.random() * a.length)];

export function offlineFortune(lang: Lang): Fortune {
  const p = PACKS[lang];
  return { luck: pick(p.luck), title: pick(p.titles), poem: pick(p.poems), advice: pick(p.advice), lucky_item: pick(p.items) };
}

export function aiEnabled(ai: AiSettings) {
  return Boolean(ai.apiKey.trim() || (ai.provider === "openai" && ai.baseUrl && !ai.baseUrl.includes("api.openai.com")));
}

/** AI 写签；失败时退回内置签文，并把原因一起返回给界面 */
export async function makeFortune(wish: string, lang: Lang, ai: AiSettings): Promise<{ fortune: Fortune; error?: ErrInfo }> {
  const fallback = offlineFortune(lang);
  if (!aiEnabled(ai)) return { fortune: fallback };
  try {
    const text = await api.aiComplete(ai, PACKS[lang].prompt(wish));
    const json = JSON.parse(text.slice(text.indexOf("{"), text.lastIndexOf("}") + 1));
    const poem = Array.isArray(json.poem) ? json.poem.map(String).slice(0, 4) : fallback.poem;
    return { fortune: { ...fallback, ...json, poem } };
  } catch (e) {
    const err = parseErr(e);
    return { fortune: fallback, error: err.code === "unknown" ? { code: "ai_failed", detail: err.detail } : err };
  }
}

export function fortuneCard(wish: string, f: Fortune, serial: number, lang: Lang, pf: PaperFont): string {
  const c = PACKS[lang].card;
  const now = new Date();
  const s = new Sheet(pf);
  const luckPx = f.luck.length > 4 ? 40 : 88;
  s.text(c.head, 28, { align: "center", weight: 700 }).text(c.sub, 16, { align: "center", weight: 400 });
  s.text(`No.${String(serial).padStart(4, "0")}   ${formatDate(now)} ${formatTime(now, "HH:mm")}`, 16, { align: "center", weight: 400 });
  s.space(4).rule().space(10);
  s.text(c.wishLabel, 16, { weight: 400 }).text(lang === "en" ? `“${wish}”` : `「${wish}」`, 22).space(4).dashed().space(14);
  s.banner(f.luck, luckPx).space(14).text(`— ${f.title} —`, 26, { align: "center", weight: 700 }).space(6);
  for (const line of f.poem) s.text(line, lang === "en" ? 20 : 26, { align: "center", lh: 1.45 });
  s.space(4).dashed().space(12).text(c.adviceLabel, 16, { weight: 400 }).text(f.advice, 22);
  s.space(4).text(`${c.itemLabel}：${f.lucky_item}`, 22).space(6).rule().space(8);
  s.text(c.foot, 16, { align: "center", weight: 400 }).text("✂ - - - - - - - - - - - - - -", 16, { align: "center", weight: 400 });
  return s.finish();
}
