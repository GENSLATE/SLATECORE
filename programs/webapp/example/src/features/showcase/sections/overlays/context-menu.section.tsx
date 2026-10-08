import {
  ContextMenu,
  ContextMenuCheckboxItem,
  ContextMenuGroup,
  ContextMenuGroupLabel,
  ContextMenuItem,
  ContextMenuPopup,
  ContextMenuRadioGroup,
  ContextMenuRadioItem,
  ContextMenuSeparator,
  ContextMenuSubmenuRoot,
  ContextMenuSubmenuTrigger,
  ContextMenuTrigger,
  Icon,
} from '@genslate/design-system';
import { Specimen } from '../../components/specimen.component';
import { EditorMenuItems } from './menu-demo';

const FILES = ['index.html', 'main.tsx', 'package.json'] as const;

export function ContextMenuSection() {
  return (
    <>
      <Specimen
        title="Context menu"
        description="Right-click (or long-press) the area. Rows are the same Menu parts."
        stageClassName="block"
        code={`<ContextMenu>\n  <ContextMenuTrigger>…</ContextMenuTrigger>\n  <ContextMenuPopup>\n    <ContextMenuItem>Rename</ContextMenuItem>\n  </ContextMenuPopup>\n</ContextMenu>`}
      >
        <ContextMenu>
          <ContextMenuTrigger className="flex h-48 items-center justify-center rounded-card border border-border border-dashed text-base text-fg-muted">
            Right-click anywhere in this area
          </ContextMenuTrigger>
          <ContextMenuPopup>
            <EditorMenuItems />
          </ContextMenuPopup>
        </ContextMenu>
      </Specimen>

      <Specimen
        title="Row types"
        description="Items, checkboxes, a radio group, labelled groups, separators and a submenu, written with the ContextMenu names."
        stageClassName="block"
        code={`<ContextMenuPopup>\n  <ContextMenuItem>Rename</ContextMenuItem>\n  <ContextMenuCheckboxItem>Pinned</ContextMenuCheckboxItem>\n  <ContextMenuGroup>\n    <ContextMenuGroupLabel>Sort by</ContextMenuGroupLabel>\n    <ContextMenuRadioGroup defaultValue="name">…</ContextMenuRadioGroup>\n  </ContextMenuGroup>\n  <ContextMenuSubmenuRoot>…</ContextMenuSubmenuRoot>\n</ContextMenuPopup>`}
      >
        <ContextMenu>
          <ContextMenuTrigger
            aria-label="Project files"
            className="flex flex-col rounded-card border border-border bg-surface-raised p-1"
          >
            {FILES.map((file) => (
              <span key={file} className="flex h-row-md items-center gap-2 px-2 text-base text-fg">
                <Icon name="codicon:file-code" size={16} className="text-fg-secondary" />
                {file}
              </span>
            ))}
            <span className="px-2 pt-1 pb-0.5 text-fg-muted text-xs">Right-click a file</span>
          </ContextMenuTrigger>
          <ContextMenuPopup>
            <ContextMenuItem icon="codicon:edit" shortcut="f2">
              Rename
            </ContextMenuItem>
            <ContextMenuItem icon="codicon:copy" shortcut="mod+d">
              Duplicate
            </ContextMenuItem>
            <ContextMenuSeparator />
            <ContextMenuCheckboxItem defaultChecked>Pinned</ContextMenuCheckboxItem>
            <ContextMenuCheckboxItem>Read only</ContextMenuCheckboxItem>
            <ContextMenuSeparator />
            <ContextMenuGroup>
              <ContextMenuGroupLabel inset>Sort by</ContextMenuGroupLabel>
              <ContextMenuRadioGroup defaultValue="name">
                <ContextMenuRadioItem value="name">Name</ContextMenuRadioItem>
                <ContextMenuRadioItem value="modified">Date modified</ContextMenuRadioItem>
                <ContextMenuRadioItem value="size">Size</ContextMenuRadioItem>
              </ContextMenuRadioGroup>
            </ContextMenuGroup>
            <ContextMenuSeparator />
            <ContextMenuSubmenuRoot>
              <ContextMenuSubmenuTrigger icon="codicon:link-external" inset={false}>
                Open with
              </ContextMenuSubmenuTrigger>
              <ContextMenuPopup>
                <ContextMenuItem>Editor</ContextMenuItem>
                <ContextMenuItem>Browser</ContextMenuItem>
                <ContextMenuItem>Terminal</ContextMenuItem>
              </ContextMenuPopup>
            </ContextMenuSubmenuRoot>
            <ContextMenuSeparator />
            <ContextMenuItem icon="codicon:trash" tone="danger">
              Move to Trash
            </ContextMenuItem>
          </ContextMenuPopup>
        </ContextMenu>
      </Specimen>
    </>
  );
}
