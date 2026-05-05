import React, { useEffect } from "react";

interface AndroidTerminalProps {
  children: React.ReactNode;
  isTablet?: boolean;
}

export const AndroidTerminal: React.FC<AndroidTerminalProps> = ({
  children,
  isTablet = false,
}) => {
  useEffect(() => {
    const vv = window.visualViewport;
    if (!vv) return;

    const handleResize = () => {
      const keyboardHeight = window.innerHeight - vv.height;
      document.documentElement.style.setProperty(
        "--keyboard-height",
        `${keyboardHeight}px`
      );
    };

    vv.addEventListener("resize", handleResize);
    // Set initial value
    handleResize();

    return () => {
      vv.removeEventListener("resize", handleResize);
    };
  }, []);

  const containerStyle: React.CSSProperties = isTablet
    ? {
        display: "grid",
        gridTemplateColumns: "1fr 1fr",
        height: "100%",
        paddingBottom: "var(--keyboard-height, 0px)",
      }
    : {
        height: "100%",
        paddingBottom: "var(--keyboard-height, 0px)",
      };

  return <div style={containerStyle}>{children}</div>;
};

export default AndroidTerminal;
