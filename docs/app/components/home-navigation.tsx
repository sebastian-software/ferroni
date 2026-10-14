const destinations = [
  ["Examples", "fr-run"],
  ["Performance", "fr-evidence"],
  ["Scanner", "fr-scanner"],
  ["Safety", "fr-safety"],
  ["Compatibility", "fr-proof"],
  ["Project status", "project-status"],
];

export function HomeNavigation() {
  return (
    <nav className="wrap fr-jump" aria-label="On this page">
      <span>Explore Ferroni</span>
      <ul>
        {destinations.map(([label, id]) => (
          <li key={id}>
            <a href={`#${id}`}>{label}</a>
          </li>
        ))}
      </ul>
    </nav>
  );
}
