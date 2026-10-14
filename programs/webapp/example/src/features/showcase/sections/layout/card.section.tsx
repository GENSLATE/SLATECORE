import {
  Badge,
  Button,
  Card,
  CardBody,
  CardFooter,
  CardHeader,
  IconButton,
} from '@genslate/design-system';
import { Specimen } from '../../components/specimen.component';

export function CardSectionPage() {
  return (
    <>
      <Specimen
        title="Variants"
        description="Flat cards: a colour step from the surface beneath plus a 1px border, never a shadow."
        stageClassName="grid grid-cols-3 items-stretch gap-5"
      >
        <Card>
          <CardHeader title="Outline" description="The default resting surface." />
          <CardBody className="text-fg-secondary">Translucent fill, border.</CardBody>
        </Card>
        <Card variant="raised">
          <CardHeader title="Raised" description="A step up, for content that sits on top." />
          <CardBody className="text-fg-secondary">surface-raised + border-subtle.</CardBody>
        </Card>
        <Card variant="sunken">
          <CardHeader title="Sunken" description="Wells and code." />
          <CardBody className="text-fg-secondary">surface-sunken + border-subtle.</CardBody>
        </Card>
      </Specimen>

      <Specimen title="Composition" stageClassName="grid grid-cols-2 items-start gap-5">
        <Card>
          <CardHeader
            title="Deploy preview"
            description="feature/nord-tokens · 2 minutes ago"
            actions={<IconButton size="sm" icon="codicon:ellipsis" label="More" />}
          />
          <CardBody className="flex items-center gap-2">
            <Badge tone="success" dot>
              Ready
            </Badge>
            <span className="text-fg-muted text-sm">Built in 38s</span>
          </CardBody>
          <CardFooter>
            <Button size="sm" variant="ghost">
              Logs
            </Button>
            <Button size="sm" variant="primary">
              Open
            </Button>
          </CardFooter>
        </Card>
        <Card interactive tabIndex={0} role="button" aria-label="Open project SLATECORE">
          <CardHeader title="SLATECORE" description="~/Projects/slatecore" />
          <CardBody className="text-fg-muted text-sm">
            Interactive cards get hover, press and a focus ring.
          </CardBody>
        </Card>
      </Specimen>
    </>
  );
}
