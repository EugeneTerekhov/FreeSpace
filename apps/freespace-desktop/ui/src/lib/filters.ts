import type { FilterSet } from '../api/dto';

/** Empty filter used by every analysis screen. */
export function emptyFilter(): FilterSet {
  return {
    text: '',
    extensions: [],
    categories: [],
    min_size: null,
    max_size: null,
    modified_after: null,
    modified_before: null,
    accessible_only: false,
    exclude_paths: [],
    name: null,
  };
}
