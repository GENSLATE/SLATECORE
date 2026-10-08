import {
  Button,
  Popover,
  PopoverClose,
  PopoverDescription,
  PopoverPopup,
  PopoverTitle,
  PopoverTrigger,
  Switch,
  TextField,
} from '@genslate/design-system';
import { Specimen } from '../../components/specimen.component';

export function PopoverSection() {
  return (
    <Specimen
      title="Popover"
      description="A floating panel on the raised popover surface: a 1px border and the one shadow in the system, with an optional arrow."
      stageClassName="gap-4 py-10"
      code={`<Popover>\n  <PopoverTrigger render={<Button />}>Share</PopoverTrigger>\n  <PopoverPopup>…</PopoverPopup>\n</Popover>`}
    >
      <Popover>
        <PopoverTrigger render={<Button leadingIcon="codicon:live-share" />}>Share</PopoverTrigger>
        <PopoverPopup className="w-72">
          <PopoverTitle>Share workspace</PopoverTitle>
          <PopoverDescription>Anyone with the link can view this session.</PopoverDescription>
          <PopoverClose />
          <div className="mt-3 flex flex-col gap-3">
            <TextField
              aria-label="Link"
              defaultValue="https://slatecore.example/s/8f2c"
              readOnly
              size="sm"
            />
            <Switch label="Allow editing" size="sm" />
          </div>
        </PopoverPopup>
      </Popover>
      <Popover>
        <PopoverTrigger render={<Button variant="ghost" trailingIcon="codicon:chevron-down" />}>
          With arrow
        </PopoverTrigger>
        <PopoverPopup arrow className="w-64">
          <PopoverTitle>main</PopoverTitle>
          <PopoverDescription>
            2 commits ahead of origin/main · last fetched 3 minutes ago.
          </PopoverDescription>
        </PopoverPopup>
      </Popover>
      <Popover>
        <PopoverTrigger render={<Button variant="ghost" />}>Placement</PopoverTrigger>
        <PopoverPopup side="right" align="start" arrow className="w-64">
          <PopoverTitle>Placed to the right</PopoverTitle>
          <PopoverDescription>
            side and align choose the edge; the popover flips when there is no room.
          </PopoverDescription>
        </PopoverPopup>
      </Popover>
    </Specimen>
  );
}
