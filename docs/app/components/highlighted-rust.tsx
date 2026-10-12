export function HighlightedRust({ caption, html }: { caption: string; html: string }) {
  return (
    <figure className="fam-code fr-rust">
      <figcaption>{caption}</figcaption>
      {/* oxlint-disable-next-line react/no-danger -- trusted HTML produced by the build's Rust highlighter. */}
      <div dangerouslySetInnerHTML={{ __html: html }} />
    </figure>
  );
}
