// 手動検証用の派生サンプルを作る（imple 6.4、T6-2）
// samples/ の前編の pptx と ppsx をそれぞれ元に、samples/derived/{pptx,ppsx}/ へ同じ構成の5種類を書き出す
//   original        原本のコピー
//   broken_link     1つの音声の rels を TargetMode="External" に書き換えたもの
//   format_mismatch 1つの音声パートを別サンプルレートで再エンコードして差し替えたもの
//   no_audio        すべての音声図形と音声メディアを取り除いたもの
//   partial_silence 一部のスライドの音声だけを取り除いたもの
// samples/derived/ は毎回作り直す。ffmpeg / ffprobe は app.settings.json のパスを使う
// 実行: bun scripts/make_samples.ts
import { copyFileSync, existsSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { unzipSync, zipSync, type Zippable } from "fflate";

const ROOT = fileURLToPath(new URL("..", import.meta.url));
const SAMPLES_DIR = join(ROOT, "samples");
const DERIVED_DIR = join(SAMPLES_DIR, "derived");
const SOURCE_MARK = "前編";

// 加工するスライド（表示順、1始まり）
const BROKEN_LINK_SLIDE = 3;
const FORMAT_MISMATCH_SLIDE = 2;
const PARTIAL_SILENCE_SLIDES = [3, 5];

const REL_AUDIO = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/audio";
const REL_MEDIA = "http://schemas.microsoft.com/office/2007/relationships/media";

type Files = Record<string, Uint8Array>;

// ---------------------------------------------------------------------------
// 最小限のXMLツリー。触らない部分は元の文字列のまま書き戻す

type XmlNode = string | XmlElement;
interface XmlElement {
  name: string;
  open: string;
  children: XmlNode[];
  close: string;
}

const TOKEN = /<(?:[^>"']|"[^"]*"|'[^']*')*>|[^<]+/g;

function parseXml(xml: string): XmlElement {
  const root: XmlElement = { name: "#root", open: "", children: [], close: "" };
  const stack = [root];
  for (const [token] of xml.matchAll(TOKEN)) {
    const top = stack[stack.length - 1];
    if (token.startsWith("</")) {
      top.close = token;
      stack.pop();
    } else if (token.startsWith("<?") || token.startsWith("<!") || !token.startsWith("<")) {
      top.children.push(token);
    } else {
      const el: XmlElement = { name: token.match(/^<([^\s/>]+)/)![1], open: token, children: [], close: "" };
      top.children.push(el);
      if (!token.endsWith("/>")) stack.push(el);
    }
  }
  if (stack.length !== 1) throw new Error("XMLの開始タグと終了タグが対応していません");
  return root;
}

function serialize(node: XmlNode): string {
  if (typeof node === "string") return node;
  return node.open + node.children.map(serialize).join("") + node.close;
}

function elements(el: XmlElement): XmlElement[] {
  return el.children.filter((c): c is XmlElement => typeof c !== "string");
}

function descendants(el: XmlElement): XmlElement[] {
  return elements(el).flatMap((c) => [c, ...descendants(c)]);
}

function attr(el: XmlElement, name: string): string | undefined {
  return el.open.match(new RegExp(`\\s${name.replace(":", "\\:")}="([^"]*)"`))?.[1];
}

/** `pred` に当てはまる要素を（子孫も含めて）取り除き、取り除いた数を返す */
function removeWhere(el: XmlElement, pred: (e: XmlElement) => boolean): number {
  let removed = 0;
  el.children = el.children.filter((c) => {
    if (typeof c === "string") return true;
    if (pred(c)) {
      removed++;
      return false;
    }
    removed += removeWhere(c, pred);
    return true;
  });
  return removed;
}

// ---------------------------------------------------------------------------
// パッケージの操作

const text = (b: Uint8Array) => new TextDecoder().decode(b);
const bytes = (s: string) => new TextEncoder().encode(s);

function relsPathOf(part: string): string {
  const i = part.lastIndexOf("/");
  return `${part.slice(0, i)}/_rels/${part.slice(i + 1)}.rels`;
}

function resolveTarget(basePart: string, target: string): string {
  const parts = target.startsWith("/") ? [] : basePart.split("/").slice(0, -1);
  for (const seg of target.split("/")) {
    if (seg === "..") parts.pop();
    else if (seg !== "." && seg !== "") parts.push(seg);
  }
  return parts.join("/");
}

/** 表示順のスライドパート（presentation.xml の sldIdLst 順） */
function slideOrder(files: Files): string[] {
  const rels = relationships(files, "ppt/presentation.xml");
  const pres = parseXml(text(files["ppt/presentation.xml"]));
  return descendants(pres)
    .filter((e) => e.name === "p:sldId")
    .map((e) => resolveTarget("ppt/presentation.xml", rels.find((r) => attr(r, "Id") === attr(e, "r:id"))!.target));
}

function relationships(files: Files, part: string) {
  const relsPart = relsPathOf(part);
  if (!files[relsPart]) return [];
  return elements(parseXml(text(files[relsPart])))
    .flatMap(elements)
    .map((r) => Object.assign(r, { target: attr(r, "Target")! }));
}

/** スライドの音声リンク（a:audioFile の r:link）が指す Relationship 要素 */
function audioLinks(files: Files, slidePart: string) {
  const slide = parseXml(text(files[slidePart]));
  const linkIds = descendants(slide)
    .filter((e) => e.name === "a:audioFile")
    .map((e) => attr(e, "r:link"));
  return relationships(files, slidePart).filter((r) => linkIds.includes(attr(r, "Id")));
}

function nthSlide(files: Files, n: number): string {
  const part = slideOrder(files)[n - 1];
  if (!part) throw new Error(`スライド${n}がありません`);
  return part;
}

/** スライドから音声図形と、それを再生するアニメーション・メディアノードを取り除く */
function stripSlideAudio(files: Files, slidePart: string) {
  const slide = parseXml(text(files[slidePart]));
  const audioPics = descendants(slide).filter(
    (e) => e.name === "p:pic" && descendants(e).some((d) => d.name === "a:audioFile"),
  );
  if (audioPics.length === 0) throw new Error(`${slidePart} に音声図形がありません`);
  const spids = new Set(
    audioPics.map((p) => attr(descendants(p).find((d) => d.name === "p:cNvPr")!, "id")!),
  );
  const targets = (e: XmlElement) =>
    descendants(e).some((d) => d.name === "p:spTgt" && spids.has(attr(d, "spid")!));

  removeWhere(slide, (e) => audioPics.includes(e));
  const timing = descendants(slide).find((e) => e.name === "p:timing");
  if (timing) {
    // メディアノード（p:audio）と、再生開始のアニメーション（presetClass="mediacall" の p:par）
    removeWhere(timing, (e) => e.name === "p:audio" && targets(e));
    removeWhere(
      timing,
      (e) =>
        e.name === "p:par" &&
        elements(e).some((ctn) => ctn.name === "p:cTn" && attr(ctn, "presetClass") === "mediacall") &&
        targets(e),
    );
    // 中身が空になったグループ（子の p:childTnLst が空の p:par）を、なくなるまで取り除く
    const emptyGroup = (e: XmlElement) =>
      e.name === "p:par" &&
      elements(e).some((ctn) => elements(ctn).some((c) => c.name === "p:childTnLst" && elements(c).length === 0));
    while (removeWhere(timing, emptyGroup) > 0);
    removeWhere(timing, (e) => e.name === "p:bldP" && spids.has(attr(e, "spid")!));
    // ほかのアニメーションが残らなければ timing ごと取り除く
    if (!descendants(timing).some((d) => d.name === "p:spTgt")) removeWhere(slide, (e) => e === timing);
  }
  const xml = serialize(slide);
  files[slidePart] = bytes(xml);

  // スライドXMLから参照されなくなった音声・画像の Relationship を取り除く
  const relsPart = relsPathOf(slidePart);
  const rels = parseXml(text(files[relsPart]));
  removeWhere(rels, (e) => {
    if (e.name !== "Relationship") return false;
    const type = attr(e, "Type")!;
    const used = xml.includes(`"${attr(e, "Id")}"`);
    return !used && (type === REL_AUDIO || type === REL_MEDIA || type.endsWith("/image"));
  });
  files[relsPart] = bytes(serialize(rels));
}

/** どの rels からも参照されなくなったメディアを取り除く */
function dropOrphanMedia(files: Files) {
  const referenced = new Set<string>();
  for (const relsPart of Object.keys(files).filter((p) => p.endsWith(".rels"))) {
    const source = relsPart.replace("/_rels/", "/").replace(/\.rels$/, "").replace(/^_rels\//, "");
    for (const r of elements(parseXml(text(files[relsPart]))).flatMap(elements)) {
      if (attr(r, "TargetMode") !== "External") referenced.add(resolveTarget(source, attr(r, "Target")!));
    }
  }
  for (const part of Object.keys(files)) {
    if (part.startsWith("ppt/media/") && !referenced.has(part)) delete files[part];
  }
}

function writePackage(files: Files, path: string) {
  // メディアは圧縮済みのため無圧縮で格納する
  const zippable: Zippable = {};
  for (const [name, data] of Object.entries(files)) {
    zippable[name] = [data, { level: name.startsWith("ppt/media/") ? 0 : 6 }];
  }
  writeFileSync(path, zipSync(zippable));
}

// ---------------------------------------------------------------------------
// 外部ツール

const settings = JSON.parse(readFileSync(join(ROOT, "app.settings.json"), "utf8")) as {
  ffmpegPath: string;
  ffprobePath: string;
};

function run(cmd: string[]): string {
  const result = Bun.spawnSync(cmd, { stdout: "pipe", stderr: "pipe" });
  if (result.exitCode !== 0) throw new Error(`${cmd[0]} が失敗しました: ${result.stderr.toString().slice(-500)}`);
  return result.stdout.toString();
}

function probe(path: string) {
  const out = run([
    settings.ffprobePath, "-v", "error", "-select_streams", "a:0",
    "-show_entries", "stream=codec_name,sample_rate,channels", "-of", "json", path,
  ]);
  const s = JSON.parse(out).streams[0];
  return { codec: s.codec_name as string, sampleRate: Number(s.sample_rate), channels: Number(s.channels) };
}

// ---------------------------------------------------------------------------
// 派生サンプル

const variants: Record<string, (files: Files, workDir: string) => string> = {
  original: () => "原本のコピー",

  broken_link(files) {
    const slidePart = nthSlide(files, BROKEN_LINK_SLIDE);
    const relsPart = relsPathOf(slidePart);
    const ids = audioLinks(files, slidePart).map((r) => attr(r, "Id"));
    const rels = parseXml(text(files[relsPart]));
    for (const r of elements(rels).flatMap(elements)) {
      if (ids.includes(attr(r, "Id"))) r.open = r.open.replace(/\s*\/>$/, ' TargetMode="External"/>');
    }
    files[relsPart] = bytes(serialize(rels));
    return `スライド${BROKEN_LINK_SLIDE}の音声リンク ${ids.join(", ")} を External に変更`;
  },

  format_mismatch(files, workDir) {
    const slidePart = nthSlide(files, FORMAT_MISMATCH_SLIDE);
    const media = resolveTarget(slidePart, audioLinks(files, slidePart)[0].target);
    const src = join(workDir, "src.m4a");
    const dst = join(workDir, "dst.m4a");
    writeFileSync(src, files[media]);
    const before = probe(src);
    const rate = before.sampleRate === 22050 ? 44100 : 22050;
    run([settings.ffmpegPath, "-v", "error", "-y", "-i", src, "-vn", "-c:a", "aac", "-ar", String(rate), dst]);
    const after = probe(dst);
    if (after.sampleRate !== rate) throw new Error(`再エンコード後のサンプルレートが ${after.sampleRate} です`);
    files[media] = new Uint8Array(readFileSync(dst));
    return `スライド${FORMAT_MISMATCH_SLIDE}の ${media} を ${before.codec}/${before.sampleRate}Hz/${before.channels}ch → ${after.codec}/${after.sampleRate}Hz/${after.channels}ch に再エンコード`;
  },

  no_audio(files) {
    const slides = slideOrder(files).filter((part) => audioLinks(files, part).length > 0);
    for (const part of slides) stripSlideAudio(files, part);
    dropOrphanMedia(files);
    return `${slides.length}枚のスライドから音声を除去`;
  },

  partial_silence(files) {
    for (const n of PARTIAL_SILENCE_SLIDES) stripSlideAudio(files, nthSlide(files, n));
    dropOrphanMedia(files);
    return `スライド${PARTIAL_SILENCE_SLIDES.join("・")}の音声を除去`;
  },
};

function main() {
  const sources = readdirSync(SAMPLES_DIR).filter((f) => f.includes(SOURCE_MARK) && /\.pp[ts]x$/.test(f));
  for (const ext of ["pptx", "ppsx"]) {
    if (!sources.some((f) => f.endsWith(`.${ext}`))) throw new Error(`samples/ に${SOURCE_MARK}の .${ext} がありません`);
  }
  rmSync(DERIVED_DIR, { recursive: true, force: true });
  const workDir = join(tmpdir(), `make_samples_${process.pid}`);
  mkdirSync(workDir, { recursive: true });
  try {
    for (const source of sources) {
      const ext = source.slice(-4);
      const outDir = join(DERIVED_DIR, ext);
      mkdirSync(outDir, { recursive: true });
      const original = new Uint8Array(readFileSync(join(SAMPLES_DIR, source)));
      console.log(`${source} → samples/derived/${ext}/`);
      for (const [name, make] of Object.entries(variants)) {
        const out = join(outDir, `${name}.${ext}`);
        if (name === "original") {
          copyFileSync(join(SAMPLES_DIR, source), out);
          console.log(`  ${name}.${ext}: 原本のコピー`);
          continue;
        }
        const files = unzipSync(original);
        const note = make(files, workDir);
        writePackage(files, out);
        console.log(`  ${name}.${ext}: ${note}`);
      }
    }
  } finally {
    if (existsSync(workDir)) rmSync(workDir, { recursive: true, force: true });
  }
}

main();
