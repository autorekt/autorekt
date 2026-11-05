// Builds autorekt.com from the repository READMEs, the only source of the site's words. For each
// language: index.html (the README rendered into template.html with that language's interface
// strings) and index.md (the README itself); plus llms.txt, a pointer for language models; the
// sitemap and robots.txt for search engines; the install scripts; and the release binaries from
// dist/ with their checksums under dl/.
// SITE_URL (default https://autorekt.com) is the address baked into the install scripts; CI
// builds the site for a local server to test the real install commands.
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { marked } from "marked";

const SITE = process.env.SITE_URL ?? "https://autorekt.com";
const BINARIES = ["linux-x64", "linux-arm64", "darwin-x64", "darwin-arm64", "windows-x64.exe", "windows-arm64.exe"].map((p) => `autorekt-${p}`);

const here = (p) => new URL(p, import.meta.url);
const read = (p) => readFileSync(here(p), "utf8");
const template = read("template.html");
const LANGS = [
  { readme: "../README.md", strings: "strings/en.json", out: "public/" },
  { readme: "../README.zh-CN.md", strings: "strings/zh-CN.json", out: "public/zh/" },
];

const esc = (s) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
const slug = (s) => s.toLowerCase().replace(/[^a-z0-9 -]/g, "").trim().replace(/ /g, "-");

// Terminal text: browsers draw block glyphs and symbols wider than ASCII, so every non-ASCII
// character gets its own one-character cell; block glyphs are clipped to it.
const cells = (html) => html.replace(/[^\x00-\x7f]/gu, (c) => `<span class="${c >= "▀" && c <= "▟" ? "g" : "s"}">${c}</span>`);
const SIGNED = /(?<![\w.:#/])([+-]\d[\d,]*\.?\d*(?:%|bp|pp|bn)?)(?![\w.])/g;

function paint(line) {
  if (line.startsWith("$ ")) return `<span class="p">$</span> <span class="cmd">${cells(esc(line.slice(2)))}</span>`;
  if (line.startsWith("## ")) return `<span class="h">${cells(esc(line))}</span>`;
  let e = esc(line).replace("&lt;- unusual", '<span class="warn">&lt;- unusual</span>');
  e = e.replace(SIGNED, (t) => (/^[+-]0(\.0+)?(%|bp|pp|bn)?$/.test(t) ? t : `<span class="${t[0] === "+" ? "up" : "dn"}">${t}</span>`));
  return cells(e.replace(/(░+)/g, '<span class="dim">$1</span>'));
}

const figure = (title, body) => `<figure class="term"><figcaption><i></i><i></i><i></i><span>${esc(title)}</span></figcaption><pre>${body}</pre></figure>`;

let strings; // interface strings of the language being built
marked.use({
  renderer: {
    code({ text, lang }) {
      if (text.startsWith("$ ")) return figure(text.split("\n")[0].slice(2), text.split("\n").map(paint).join("\n"));
      if (lang === "text" || lang === "sh") return `<div class="copybox"><pre>${esc(text)}</pre><button type="button" data-copy>${strings.copy}</button></div>`;
      if (lang === "markdown") return figure("report.md", text.split("\n").map((l) => (l.startsWith("## ") ? `<span class="h">${esc(l)}</span>` : cells(esc(l)))).join("\n"));
      throw new Error(`README: no rendering for a code block in '${lang}'`);
    },
    html({ text }) {
      // <details> keeps long output folded on GitHub; the site shows it in scroll boxes instead.
      if (/^\s*<\/?(details|summary)/.test(text)) return "";
      throw new Error(`README: unexpected HTML block: ${text.slice(0, 60)}`);
    },
    list(token) {
      // A list whose items all lead with bold text is a grid of cards: title, text, example.
      if (!token.items.every((i) => i.text.startsWith("**"))) return false;
      const cards = token.items.map((i) => {
        const html = this.parser.parseInline(i.tokens[0].tokens);
        const [, title, rest] = html.match(/^<strong>(.*?)<\/strong>\s*(.*)$/s);
        const [, body, example] = rest.match(/^(.*?)\s*(<code>autorekt [^<]*<\/code>)?$/s);
        return `<article><h3>${title}</h3><p>${body}</p>${example ? `<p class="ex">${example}</p>` : ""}</article>`;
      });
      return `<div class="cards">${cards.join("")}</div>`;
    },
  },
});

// The README as an intro (before the first ## heading) and ## sections.
function parse(readme) {
  const tokens = marked.lexer(readme);
  const sections = [];
  const intro = [];
  for (const t of tokens) {
    if (t.type === "heading" && t.depth === 2) sections.push({ title: t.text, body: [] });
    else if (sections.length) sections.at(-1).body.push(t);
    else intro.push(t);
  }
  // The first paragraph links the README's languages; the site has its own switch.
  const [, tagline, lead] = intro.filter((t) => t.type === "paragraph");
  return { tokens, sections, intro, tagline, lead };
}

function build({ readme, strings: stringsFile, out }, ids) {
  strings = JSON.parse(read(stringsFile));
  const text = read(readme);
  const { tokens, sections, intro, tagline, lead } = parse(text);
  const render = (ts) => marked.parser(Object.assign(ts, { links: tokens.links }));

  // Within a section, each ### subsection is words on the left, its output on the right.
  function sectionBody(body) {
    const out = [];
    let duo = null;
    for (const t of body) {
      if (t.type === "heading" && t.depth === 3) {
        duo = { copy: [t], media: [] };
        out.push(duo);
      } else if (duo && (t.type === "paragraph" || t.type === "space")) {
        (duo.media.length ? duo.media : duo.copy).push(t);
      } else if (duo) {
        duo.media.push(t);
      } else {
        out.push(t);
      }
    }
    return out.map((x) => (x.copy ? `<div class="duo"><div class="copy">${render(x.copy)}</div><div class="stack">${render(x.media)}</div></div>` : render([x]))).join("\n");
  }

  if (sections.length !== ids.length) throw new Error(`${readme} has ${sections.length} sections; README.md has ${ids.length}`);
  const main = sections
    .slice(0, -1) // the last section is the license, shown in the footer
    .map((s, i) => `<section id="${ids[i]}"${i % 2 ? "" : ' class="band"'}><div class="wrap"><h2>${esc(s.title)}</h2>\n${sectionBody(s.body)}</div></section>`)
    .join("\n");
  const license = sections.at(-1).body.find((t) => t.type === "paragraph").text;

  let html = template
    .replace("{{TAGLINE}}", marked.parseInline(tagline.text).replace(/<\/?strong>/g, ""))
    .replace("{{LEAD}}", marked.parseInline(lead.text))
    .replace("{{JOKE}}", render(intro.filter((t) => t.type === "blockquote")))
    .replace("{{HERO}}", render(intro.filter((t) => t.type === "code")))
    .replace("{{SECTIONS}}", main)
    .replace("{{LICENSE}}", license)
    .replace(/\{\{s\.(\w+)\}\}/g, (_, k) => {
      if (!(k in strings)) throw new Error(`${stringsFile} has no '${k}'`);
      return strings[k];
    });
  for (const [, id] of html.matchAll(/href="#([^"]+)"/g)) {
    if (!html.includes(`id="${id}"`)) throw new Error(`template links to #${id}, which ${readme} does not have`);
  }
  mkdirSync(here(out), { recursive: true });
  writeFileSync(here(`${out}index.html`), html);
  writeFileSync(here(`${out}index.md`), text);
  console.log(`built ${out}index.html (${html.length} bytes) and ${out}index.md`);
  return { tagline, lead };
}

// Section ids come from the English headings, so every language shares the same anchors.
const english = parse(read(LANGS[0].readme));
const ids = english.sections.map((s) => slug(s.title));
for (const l of LANGS) build(l, ids);

writeFileSync(
  here("public/llms.txt"),
  `# autorekt\n\n> ${english.tagline.text.replace(/\*\*/g, "")} ${english.lead.text.replace(/\s+/g, " ")}\n\n` +
    `- [README as Markdown](${SITE}/index.md): what autorekt does, install, real output, commands and sources\n` +
    `- [README in Chinese](${SITE}/zh/index.md)\n` +
    `- [Source code](https://github.com/autorekt/autorekt)\n`,
);
console.log("built public/llms.txt");

const pages = LANGS.map((l) => `${SITE}/${l.out.slice("public/".length)}`);
const alternates = LANGS.map((l, i) => `<xhtml:link rel="alternate" hreflang="${JSON.parse(read(l.strings)).lang}" href="${pages[i]}"/>`).join("");
writeFileSync(
  here("public/sitemap.xml"),
  `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">\n` +
    pages.map((p) => `<url><loc>${p}</loc>${alternates}</url>\n`).join("") +
    `</urlset>\n`,
);
writeFileSync(here("public/robots.txt"), `User-agent: *\nAllow: /\n\nSitemap: ${SITE}/sitemap.xml\n`);
console.log("built public/sitemap.xml and public/robots.txt");

for (const f of ["install.sh", "install.ps1", "install.cmd"]) {
  const text = read(`install/${f}`).replaceAll("{{SITE}}", SITE);
  writeFileSync(here(`public/${f}`), f.endsWith(".cmd") ? text.replace(/\r?\n/g, "\r\n") : text);
}
console.log(`built install scripts for ${SITE}`);

mkdirSync(here("public/dl"), { recursive: true });
const sums = BINARIES.map((f) => {
  if (!existsSync(here(`dist/${f}`))) throw new Error(`site/dist/${f} is missing; fetch the release binaries with: gh release download -R autorekt/autorekt -p 'autorekt-*' -D site/dist`);
  const data = readFileSync(here(`dist/${f}`));
  writeFileSync(here(`public/dl/${f}`), data);
  return `${createHash("sha256").update(data).digest("hex")}  ${f}\n`;
});
writeFileSync(here("public/dl/SHA256SUMS"), sums.join(""));
console.log(`built dl/ with ${BINARIES.length} binaries`);
