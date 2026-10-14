import { Button, PasswordField, type PasswordFieldProps } from '@genslate/design-system';
import { type ComponentType, type FormEvent, useRef, useState } from 'react';
import { PropsTable } from '../../components/props-table.component';
import { Specimen } from '../../components/specimen.component';
import { StateMatrix } from '../../components/state-matrix.component';

const HOVER = 'border-border-strong';
const FOCUS = 'border-focus outline-focus';
const SIZES = ['sm', 'md', 'lg'] as const;

type DemoProps = Omit<PasswordFieldProps, 'value' | 'onChange'> & { initial?: string };

/** A controlled PasswordField: the demo owns the value in React state. */
function Demo({ initial = '', ...props }: DemoProps) {
  const [value, setValue] = useState(initial);
  return <PasswordField {...props} value={value} onChange={setValue} />;
}

/**
 * An uncontrolled PasswordField (no `value`, no `onChange`): the text lives only in the DOM
 * input. The demo writes its sample text straight into the input, never through props.
 */
function UncontrolledDemo({ initial = '', ...props }: DemoProps) {
  const prefill = (input: HTMLInputElement | null) => {
    if (input !== null && input.value === '') input.value = initial;
  };
  return <PasswordField {...props} ref={prefill} />;
}

/** Stands in for the one consumer of the secret (an IPC call in the launcher). */
function unlock(secret: string): boolean {
  return secret.length > 0;
}

/** The pattern for secrets: read the input once on submit, hand it on, clear it. */
function UncontrolledForm() {
  const ref = useRef<HTMLInputElement>(null);
  const [error, setError] = useState<string>();
  const [status, setStatus] = useState('Nothing submitted yet.');
  const submit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const input = ref.current;
    if (input === null) return;
    if (input.value === '') {
      setError('Enter the vault password.');
      return;
    }
    unlock(input.value);
    input.value = '';
    setStatus('Read once, then cleared.');
  };
  return (
    <form className="flex flex-col gap-3" onSubmit={submit}>
      <PasswordField
        ref={ref}
        label="Vault password"
        description="Uncontrolled: React never holds the secret."
        error={error}
        onInput={() => setError(undefined)}
      />
      <div className="flex items-center gap-3">
        <Button type="submit" size="sm">
          Unlock
        </Button>
        <p role="status" className="text-fg-muted text-sm">
          {status}
        </p>
      </div>
    </form>
  );
}

/** One state-matrix row per size, for either mode. */
function stateRows(Field: ComponentType<DemoProps>) {
  return SIZES.map((size) => ({
    label: size.toUpperCase(),
    cells: [
      <Field
        className="w-32"
        key="rest"
        label={hidden('Rest')}
        size={size}
        placeholder="Password"
      />,
      <Field
        className="w-32"
        key="hover"
        label={hidden('Hover')}
        size={size}
        placeholder="Password"
        controlClassName={HOVER}
      />,
      <Field
        className="w-32"
        key="focus"
        label={hidden('Focus')}
        size={size}
        initial="focused"
        controlClassName={FOCUS}
      />,
      <Field
        className="w-32"
        key="filled"
        label={hidden('Filled')}
        size={size}
        initial="hunter2-value"
      />,
      <Field
        className="w-32"
        key="invalid"
        label={hidden('Invalid')}
        size={size}
        initial="short"
        invalid
      />,
      <Field
        className="w-32"
        key="disabled"
        label={hidden('Disabled')}
        size={size}
        initial="locked-value"
        disabled
      />,
    ],
  }));
}

const STATES = ['Rest', 'Hover', 'Focus', 'Filled', 'Invalid', 'Disabled'];

const hidden = (text: string) => <span className="sr-only">{text}</span>;

function Confirmation() {
  const [passphrase, setPassphrase] = useState('correct horse');
  const [again, setAgain] = useState('correct hors');
  const mismatch = again.length > 0 && again !== passphrase;
  return (
    <>
      <PasswordField
        label="New passphrase"
        description="At least 12 characters."
        value={passphrase}
        onChange={setPassphrase}
      />
      <PasswordField
        label="Repeat passphrase"
        value={again}
        onChange={setAgain}
        error={mismatch ? 'The passphrases do not match.' : undefined}
      />
    </>
  );
}

export function PasswordFieldSection() {
  return (
    <>
      <Specimen
        title="Masked until revealed"
        description="The value stays hidden behind dots. The eye toggle shows it, and a mouse click keeps the caret in the field."
        stageClassName="grid grid-cols-2 items-start gap-6"
        code={`const [passphrase, setPassphrase] = useState('');\n\n<PasswordField label="Passphrase" value={passphrase} onChange={setPassphrase} />`}
      >
        <Demo
          label="Passphrase"
          description="No browser autofill, suggestions or spell-check."
          initial="correct horse battery staple"
        />
        <Demo label="Vault key" placeholder="Enter your vault key" />
      </Specimen>

      <StateMatrix caption="Password field states" columns={STATES} rows={stateRows(Demo)} />

      <Specimen
        title="Uncontrolled for secrets"
        description="Leave out value and onChange and the secret stays only in the DOM input: not in React state, not in Base UI’s field state. Read it through ref on submit, hand it on once, then clear the input."
        stageClassName="grid grid-cols-2 items-start gap-6"
        code={`const ref = useRef<HTMLInputElement>(null);\n\nfunction submit() {\n  const input = ref.current;\n  if (input === null) return;\n  unlock(input.value); // the one place the secret goes\n  input.value = '';\n}\n\n<PasswordField ref={ref} label="Vault password" />`}
      >
        <UncontrolledForm />
      </Specimen>

      <StateMatrix
        caption="Uncontrolled password field states"
        columns={STATES}
        rows={stateRows(UncontrolledDemo)}
      />

      <Specimen
        title="Validation and variants"
        description="Pair two fields for confirmation, hide the toggle for write-only secrets, or rename the toggle for assistive technology."
        stageClassName="grid grid-cols-3 items-start gap-6"
      >
        <div className="flex flex-col gap-4">
          <Confirmation />
        </div>
        <Demo
          label="Write-only token"
          description="No reveal toggle."
          revealable={false}
          initial="ghp_not_shown"
        />
        <Demo
          label="Recovery phrase"
          description="Custom toggle name."
          labels={{ show: 'Reveal phrase' }}
          initial="correct horse battery"
        />
      </Specimen>

      <PropsTable
        rows={[
          {
            name: 'value / onChange',
            type: 'string / (value: string) => void',
            description:
              'Optional, both or neither. Given, the field is controlled. Left out, it is uncontrolled: the secret lives only in the DOM input (read it through ref, then clear it), never in React state. Use that for passwords.',
          },
          {
            name: 'ref',
            type: 'Ref<HTMLInputElement>',
            description: 'The input itself: how an uncontrolled field is read and cleared.',
          },
          {
            name: 'label',
            type: 'ReactNode',
            description: 'Visible label and the input’s accessible name.',
          },
          {
            name: 'error',
            type: 'ReactNode',
            description: 'Message under the field; marks the input invalid while set.',
          },
          {
            name: 'revealable',
            type: 'boolean',
            default: 'true',
            description: 'Shows the eye toggle that reveals the value.',
          },
          {
            name: 'labels',
            type: '{ show?: string }',
            default: "'Show password'",
            description:
              'Accessible name of the toggle; it stays the same in both states and aria-pressed tells which one is active.',
          },
          {
            name: 'size',
            type: "'sm' | 'md' | 'lg'",
            default: "'md'",
            description: 'Control height 24 / 28 / 32, like TextField.',
          },
        ]}
      />
    </>
  );
}
