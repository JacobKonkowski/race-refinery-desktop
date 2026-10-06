/**
 * Cross-feature navigation without coupling features to the shell: a feature
 * asks for another feature by id and the shell switches to it.
 */
const NAVIGATE_EVENT = "race-refinery:navigate";

export function navigateToFeature(id: string) {
  window.dispatchEvent(new CustomEvent<string>(NAVIGATE_EVENT, { detail: id }));
}

export function onNavigateToFeature(callback: (id: string) => void): () => void {
  const listener = (e: Event) => callback((e as CustomEvent<string>).detail);
  window.addEventListener(NAVIGATE_EVENT, listener);
  return () => window.removeEventListener(NAVIGATE_EVENT, listener);
}
