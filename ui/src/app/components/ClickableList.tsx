/** biome-ignore-all lint/a11y/noStaticElementInteractions: Native OpenTUI rows have no DOM/ARIA; the shell retains keyboard navigation and every click selects through the same store. */
import type { ScrollBoxRenderable } from "@opentui/core";
import { useEffect, useRef } from "react";
import { theme } from "../theme";

export function ClickableList({
  id,
  options,
  selectedIndex,
  onSelect,
}: {
  id: string;
  options: Array<{ name: string }>;
  selectedIndex: number;
  onSelect: (index: number) => void;
}) {
  const scroll = useRef<ScrollBoxRenderable | null>(null);
  useEffect(() => {
    scroll.current?.scrollChildIntoView(`${id}-row-${selectedIndex}`);
  }, [id, selectedIndex]);
  return (
    <scrollbox id={id} ref={scroll} flexGrow={1} minHeight={0} scrollY>
      {options.map((option, index) => (
        <text
          id={`${id}-row-${index}`}
          key={option.name}
          width="100%"
          height={1}
          truncate
          fg={index === selectedIndex ? theme.primary : theme.text}
          bg={index === selectedIndex ? theme.element : theme.panel}
          onMouseDown={(event) => {
            event.stopPropagation();
            if (event.button === 0) onSelect(index);
          }}
        >{`${index === selectedIndex ? "▶ " : "  "}${option.name}`}</text>
      ))}
    </scrollbox>
  );
}
