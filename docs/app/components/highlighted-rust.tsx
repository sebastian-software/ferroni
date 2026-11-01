import { FerraCode } from "ferramenta-family/ferra";

export function HighlightedRust({ caption, html }: { caption: string; html: string }) {
  return (
    <FerraCode caption={caption}>
      {/* oxlint-disable-next-line react/no-danger -- Trusted inline spans from the build-time code highlighter. */}
      <span dangerouslySetInnerHTML={{ __html: html }} />
    </FerraCode>
  );
}
