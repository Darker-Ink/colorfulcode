import React, { useEffect, useState } from "react";

export default function Clock({ label = "Time", locale }) {
  const [now, setNow] = useState(() => new Date());

  useEffect(() => {
    const id = setInterval(() => setNow(new Date()), 1000);
    return () => clearInterval(id);
  }, []);

  return (
    <>
      <h2 style={{ fontWeight: 600 }}>{label}</h2>
      {/* Updates every second */}
      <time dateTime={now.toISOString()}>{now.toLocaleTimeString(locale)}</time>
      <Badge.Item tone="info" {...{ size: "sm" }} />
    </>
  );
}
