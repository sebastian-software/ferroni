import { FerraExample } from "ferramenta-family/ferra";

type PatternExampleProps = {
  example: {
    title: string;
    explanation: string;
    before: string;
    matched: string;
    after: string;
    pattern: string;
  };
};

export function PatternExample({ example }: PatternExampleProps) {
  return (
    <FerraExample title={example.title} introduction={example.explanation} source={example.pattern}>
      <p>
        <code>
          {example.before}
          <mark>{example.matched}</mark>
          {example.after}
        </code>
      </p>
      <p>
        Matched text: <code>{example.matched}</code>
      </p>
    </FerraExample>
  );
}
