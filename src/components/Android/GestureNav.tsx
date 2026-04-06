import { useRef, useCallback } from 'react';

interface GestureNavProps {
  readonly children: React.ReactNode;
  readonly onSwipeLeft?: () => void;
  readonly onSwipeRight?: () => void;
  readonly onSwipeUp?: () => void;
  readonly onSwipeDown?: () => void;
  readonly sensitivity?: number;
}

export default function GestureNav({
  children,
  onSwipeLeft,
  onSwipeRight,
  onSwipeUp,
  onSwipeDown,
  sensitivity = 50,
}: Readonly<GestureNavProps>) {
  const touchStart = useRef<{ x: number; y: number } | null>(null);

  const handleTouchStart = useCallback((e: React.TouchEvent) => {
    const touch = e.touches[0];
    touchStart.current = { x: touch.clientX, y: touch.clientY };
  }, []);

  const handleTouchEnd = useCallback(
    (e: React.TouchEvent) => {
      if (!touchStart.current) return;

      const touch = e.changedTouches[0];
      const dx = touch.clientX - touchStart.current.x;
      const dy = touch.clientY - touchStart.current.y;
      const absDx = Math.abs(dx);
      const absDy = Math.abs(dy);

      if (absDx < sensitivity && absDy < sensitivity) {
        touchStart.current = null;
        return;
      }

      if (absDx > absDy) {
        if (dx > 0) {
          onSwipeRight?.();
        } else {
          onSwipeLeft?.();
        }
      } else if (dy > 0) {
        onSwipeDown?.();
      } else {
        onSwipeUp?.();
      }

      touchStart.current = null;
    },
    [onSwipeLeft, onSwipeRight, onSwipeUp, onSwipeDown, sensitivity]
  );

  return (
    <div
      onTouchStart={handleTouchStart}
      onTouchEnd={handleTouchEnd}
      className="h-full w-full"
    >
      {children}
    </div>
  );
}
