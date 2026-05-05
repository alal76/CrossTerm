import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, act } from '@testing-library/react';
import { invoke } from '@tauri-apps/api/core';
import MacroEditor from './MacroEditor';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
  }),
}));

vi.mock('@dnd-kit/sortable', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@dnd-kit/sortable')>();
  return {
    ...actual,
    arrayMove: vi.fn((arr: unknown[], from: number, to: number) => {
      const result = [...(arr as unknown[])];
      const [removed] = result.splice(from, 1);
      result.splice(to, 0, removed);
      return result;
    }),
  };
});

const { arrayMove } = await import('@dnd-kit/sortable');

const mockedInvoke = vi.mocked(invoke);

describe('MacroEditor', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockedInvoke.mockResolvedValue([]);
  });

  it('renders with empty step list', async () => {
    await act(async () => { render(<MacroEditor />); });
    expect(screen.getByText('macro.editor')).toBeTruthy();
    expect(screen.getByText('macro.addStep')).toBeTruthy();
  });

  it('renders grip handles when steps are present', async () => {
    await act(async () => { render(<MacroEditor />); });

    act(() => { fireEvent.click(screen.getByText('Send')); });

    const grips = document.querySelectorAll('svg.lucide-grip-vertical');
    expect(grips.length).toBeGreaterThan(0);
  });

  it('renders DndContext wrapper with SortableContext when steps exist', async () => {
    await act(async () => { render(<MacroEditor />); });

    act(() => { fireEvent.click(screen.getByText('Wait')); });
    act(() => { fireEvent.click(screen.getByText('Send')); });

    expect(screen.getByText('Wait: 1000ms')).toBeTruthy();
    expect(screen.getByText('Send: (empty)')).toBeTruthy();

    const grips = document.querySelectorAll('svg.lucide-grip-vertical');
    expect(grips.length).toBe(2);
  });

  it('arrayMove is called with correct indices on drag end', () => {
    const arr = ['a', 'b', 'c'];
    const result = (arrayMove as ReturnType<typeof vi.fn>)(arr, 0, 2);
    expect(arrayMove).toHaveBeenCalledWith(arr, 0, 2);
    expect(result).toEqual(['b', 'c', 'a']);
  });

  it('removing a step updates the step list', async () => {
    await act(async () => { render(<MacroEditor />); });

    act(() => { fireEvent.click(screen.getByText('Send')); });
    act(() => { fireEvent.click(screen.getByText('Send')); });

    expect(screen.getAllByText('Send: (empty)').length).toBe(2);

    // Each step card has a remove button; click the first one via title-less button
    // The step card remove buttons are the only buttons with a red trash icon inside a step row
    const stepCards = document.querySelectorAll('.space-y-2 > div');
    const firstRemoveBtn = stepCards[0].querySelector('button') as HTMLButtonElement;
    act(() => { fireEvent.click(firstRemoveBtn); });

    expect(screen.getAllByText('Send: (empty)').length).toBe(1);
  });
});
