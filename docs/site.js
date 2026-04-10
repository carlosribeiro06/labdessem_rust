const revealElements = [...document.querySelectorAll(".section, .metric-card, .stack-card, .workflow-step, .scope-card, .panel")];

for (const element of revealElements) {
  element.classList.add("reveal");
}

const observer = new IntersectionObserver(
  (entries) => {
    for (const entry of entries) {
      if (entry.isIntersecting) {
        entry.target.classList.add("is-visible");
        observer.unobserve(entry.target);
      }
    }
  },
  { threshold: 0.12 }
);

for (const element of revealElements) {
  observer.observe(element);
}
