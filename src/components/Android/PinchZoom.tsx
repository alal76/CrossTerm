import { useRef, useState, useCallback } from 'react';
import clsx from 'clsx';

interface PinchZoomProps {
  children: React.ReactNode;
  minZoom?: number;
  maxZoom?: number;
  className?: string;
}

export default function PinchZoom({
  children,
  minZoom = 0.5,
  maxZoom = 3,
  className,
}: Readonly<PinchZoomProps>) {
  const [scale, setScale] = useState(1);
  const initialDistance = useRef<number | null>(null);
  const initialScale = useRef(1);

  const getDistance = (touches: React.TouchList): number => {
    const [a, b] = [touches[0], touches[1]];
    return Math.hypot(b.clientX - a.clientX, b.clientY - a.clientY);
  };

  const handleTouchStart = useCallback(
    (e: React.TouchEvent) => {
      if (e.touches.length === 2) {
        initialDistance.current = getDistance(e.touches);
        initialScale.current = scale;
      }
    },
    [scale]
  );

  const handleTouchMove = useCallback(
    (e: React.TouchEvent) => {
      if (e.touches.length === 2 && initialDistance.current !== null) {
        const currentDistance = getDistance(e.touches);
        const ratio = currentDistance / initialDistance.current;
        const newScale = Math.min(maxZoom, Math.max(minZoom, initialScale.current * ratio));
        setScale(newScale);
      }
    },
    [minZoom, maxZoom]
  );

  const handleTouchEnd = useCallback(() => {
    initialDistance.current = null;
  }, []);

  return (
    <div
      className={clsx('overflow-hidden', className)}
      onTouchStart={handleTouchStart}
      onTouchMove={handleTouchMove}
      onTouchEnd={handleTouchEnd}
      onTouchCancel={handleTouchEnd}
    >
      <div
        style={{
          transform: `scale(${scale})`,
          transformOrigin: 'center center',
          transition: 'transform 0.05s ease-out',
        }}
      >
        {children}
      </div>
    </div>
  );
}
