type PatternExampleProps = {
  example: { title: string; explanation: string; before: string; matched: string; after: string };
  html: string;
};

export function PatternExample({ example, html }: PatternExampleProps) {
  return (
    <article className="fr-example">
      <h3>{example.title}</h3>
      <p>{example.explanation}</p>
      <dl>
        <div className="fr-example-pattern">
          <dt>Pattern</dt>
          {/* oxlint-disable-next-line react/no-danger -- trusted build-time regex highlighting. */}
          <dd dangerouslySetInnerHTML={{ __html: html }} />
        </div>
        <div className="fr-example-input">
          <dt>Input text · highlighted match</dt>
          <dd>
            <code>
              {example.before}
              <mark>{example.matched}</mark>
              {example.after}
            </code>
          </dd>
        </div>
        <div className="fr-example-match">
          <dt>Matched text</dt>
          <dd>
            <code>{example.matched}</code>
          </dd>
        </div>
      </dl>
    </article>
  );
}
