import { useEffect, useState } from 'react';

/**
 * Resolves `prefers-reduced-motion` and follows OS changes.
 *
 * Components that animate numbers or reveals call this instead of reading the
 * media query once, so a user changing the preference does not need a reload.
 */
export function usePrefersReducedMotion(): boolean {
  const [reduced, setReduced] = useState(false);

  useEffect(() => {
    if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') {
      return undefined;
    }

    const query = window.matchMedia('(prefers-reduced-motion: reduce)');
    const apply = (matches: boolean) => {
      setReduced(matches);
    };
    apply(query.matches);
    const onChange = (event: MediaQueryListEvent) => {
      apply(event.matches);
    };
    query.addEventListener('change', onChange);

    return () => {
      query.removeEventListener('change', onChange);
    };
  }, []);

  return reduced;
}
