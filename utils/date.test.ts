import { formatDate } from './date';

describe('formatDate', () => {
  it('formats ISO string to dd/mm/yyyy', () => {
    const iso = '2024-01-15T00:00:00Z';
    expect(formatDate(iso)).toBe('15/01/2024');
  });
});
