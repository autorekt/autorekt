// Builds autorekt.com from the repository README, the only source of the site's words:
// public/index.html (README rendered into template.html), public/index.md (the README itself)
// and public/llms.txt (a pointer for language models).
import { readFileSync, writeFileSync } from "node:fs";
import { marked } from "marked";

const here = (p) => new URL(p, import.meta.url);
const readme = readFileSync(here("../README.md"), "utf8");
const template = readFileSync(here("template.html"), "utf8");

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

const renderer = {
  code({ text, lang }) {
    if (text.startsWith("$ ")) return figure(text.split("\n")[0].slice(2), text.split("\n").map(paint).join("\n"));
    if (lang === "text" || lang === "sh") return `<div class="copybox"><pre>${esc(text)}</pre><button type="button" data-copy>Copy</button></div>`;
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
};
marked.use({ renderer });

const tokens = marked.lexer(readme);
const render = (ts) => marked.parser(Object.assign(ts, { links: tokens.links }));

// Split into the intro (before the first ## heading) and ## sections.
const sections = [];
let intro = [];
for (const t of tokens) {
  if (t.type === "heading" && t.depth === 2) sections.push({ title: t.text, body: [] });
  else if (sections.length) sections.at(-1).body.push(t);
  else intro.push(t);
}
intro = intro.filter((t) => t.type !== "space" && !(t.type === "heading" && t.depth === 1));
const [tagline, lead] = intro.filter((t) => t.type === "paragraph");
const heroTerminals = render(intro.filter((t) => t.type === "code"));
const joke = render(intro.filter((t) => t.type === "blockquote"));

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

const license = sections.find((s) => s.title === "License");
const main = sections
  .filter((s) => s !== license)
  .map((s, i) => `<section id="${slug(s.title)}"${i % 2 ? "" : ' class="band"'}><div class="wrap"><h2>${esc(s.title)}</h2>\n${sectionBody(s.body)}</div></section>`)
  .join("\n");

let html = template
  .replace("{{TAGLINE}}", marked.parseInline(tagline.text).replace(/<\/?strong>/g, ""))
  .replace("{{LEAD}}", marked.parseInline(lead.text))
  .replace("{{JOKE}}", joke)
  .replace("{{HERO}}", heroTerminals)
  .replace("{{SECTIONS}}", main)
  .replace("{{LICENSE}}", marked.parseInline(license.body.find((t) => t.type === "paragraph").text));
for (const [, id] of html.matchAll(/href="#([^"]+)"/g)) {
  if (!html.includes(`id="${id}"`)) throw new Error(`template links to #${id}, which the README does not have`);
}
writeFileSync(here("public/index.html"), html);
writeFileSync(here("public/index.md"), readme);
writeFileSync(
  here("public/llms.txt"),
  `# autorekt\n\n> ${tagline.text.replace(/\*\*/g, "")} ${lead.text.replace(/\s+/g, " ")}\n\n` +
    `- [README as Markdown](https://autorekt.com/index.md): what autorekt does, install, real output, commands and sources\n` +
    `- [Source code](https://github.com/autorekt/autorekt)\n`,
);
console.log(`built index.html (${html.length} bytes), index.md, llms.txt`);
