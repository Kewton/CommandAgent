type MarkdownBlock =
  | { kind: "heading"; level: 1 | 2; text: string }
  | { kind: "list"; items: string[] }
  | { kind: "paragraph"; text: string };

export function GateCardMarkdown({ markdown }: { markdown: string }) {
  const blocks = parseGateCard(markdown);
  return (
    <div
      aria-label="Gate 1 の確認内容"
      className="gate-card-markdown"
      data-testid="gate-one-card-markdown"
    >
      {blocks.map((block, index) => {
        if (block.kind === "heading") {
          return block.level === 1
            ? <h2 key={index}>{block.text}</h2>
            : <h3 key={index}>{block.text}</h3>;
        }
        if (block.kind === "list") {
          return (
            <ul key={index}>
              {block.items.map((item, itemIndex) => <li key={`${itemIndex}-${item}`}>{item}</li>)}
            </ul>
          );
        }
        return <p key={index}>{block.text}</p>;
      })}
    </div>
  );
}

function parseGateCard(markdown: string): MarkdownBlock[] {
  const blocks: MarkdownBlock[] = [];
  let list: string[] = [];
  let paragraph: string[] = [];

  const flushList = () => {
    if (list.length === 0) return;
    blocks.push({ kind: "list", items: list });
    list = [];
  };
  const flushParagraph = () => {
    if (paragraph.length === 0) return;
    blocks.push({ kind: "paragraph", text: paragraph.join(" ") });
    paragraph = [];
  };

  for (const line of markdown.split("\n")) {
    const trimmed = line.trim();
    if (trimmed === "") {
      flushList();
      flushParagraph();
      continue;
    }
    const heading = /^(#{1,2})\s+(.+)$/.exec(trimmed);
    if (heading !== null) {
      flushList();
      flushParagraph();
      blocks.push({
        kind: "heading",
        level: heading[1].length === 1 ? 1 : 2,
        text: heading[2],
      });
      continue;
    }
    if (trimmed.startsWith("- ")) {
      flushParagraph();
      list.push(trimmed.slice(2));
      continue;
    }
    flushList();
    paragraph.push(trimmed);
  }
  flushList();
  flushParagraph();
  return blocks;
}
