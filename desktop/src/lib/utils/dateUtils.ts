export const dateUtils = {
  isSameDay(date1: Date, date2: Date): boolean {
    return (
      date1.getFullYear() === date2.getFullYear() &&
      date1.getMonth() === date2.getMonth() &&
      date1.getDate() === date2.getDate()
    );
  },

  startOfWeek(date: Date): Date {
    const d = new Date(date);
    const day = d.getDay();
    // By default start at Sunday
    const diff = day === 0 ? 6 : day - 1;
    d.setDate(d.getDate() - diff);
    d.setHours(0, 0, 0, 0);
    return d;
  },

  startOfMonth(date: Date): Date {
    const d = new Date(date.getFullYear(), date.getMonth(), 1);
    d.setHours(0, 0, 0, 0);
    return d;
  },

  getMonthGrid(date: Date): Date[] {
    const start = this.startOfMonth(date);
    const firstDayIndex = start.getDay();
    // By default start at Sunday
    const adjustedIndex = firstDayIndex === 0 ? 6 : firstDayIndex - 1;
    const gridStart = new Date(start);
    gridStart.setDate(start.getDate() - adjustedIndex);

    return Array.from({ length: 42 }, (_, i) => {
      const d = new Date(gridStart);
      d.setDate(gridStart.getDate() + i);
      return d;
    });
  },

  getWeekDays(date: Date): Date[] {
    const start = this.startOfWeek(date);
    return Array.from({ length: 7 }, (_, i) => {
      const d = new Date(start);
      d.setDate(start.getDate() + i);
      return d;
    });
  },

  /**
   * Converts an API timestamp (RFC3339, UTC) into the exact string an
   * `<input type="datetime-local">` accepts: `YYYY-MM-DDTHH:mm`, in *local*
   * time, no seconds, no zone suffix.
   *
   * Deliberately not `toISOString().slice(0, 16)`, which is the obvious
   * thing and is wrong twice over: it yields UTC rather than local time, so
   * the form shows the wrong hour for any user not on UTC. An input given a
   * value it can't parse renders blank without throwing, so this fails
   * silently when it fails.
   */
  toDatetimeLocalValue(isoString: string): string {
    const d = new Date(isoString);
    if (Number.isNaN(d.getTime())) return '';
    const pad = (n: number) => String(n).padStart(2, '0');
    return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
  }
};

export function formatDate(dateString: string) {
  const date = new Date(dateString);
  return date.toLocaleDateString('en-US', {
    weekday: 'short',
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit'
  });
}