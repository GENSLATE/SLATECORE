import { PasswordField, type PasswordFieldProps } from '@genslate/design-system';
import { useState } from 'react';
import { PropsTable } from '../../components/props-table.component';
import { Specimen } from '../../components/specimen.component';
import { StateMatrix } from '../../components/state-matrix.component';

const HOVER = 'border-border-strong';
const FOCUS = 'border-focus outline-focus';
const SIZES = ['sm', 'md', 'lg'] as const;

type DemoProps = Omit<PasswordFieldProps, 'value' | 'onChange'> & { initial?: string };

/** A PasswordField that owns its secret (the component itself is always controlled). */
function Demo({ initial = '', ...props }: DemoProps) {
  const [value, setValue] = useState(initial);
  return <PasswordField {...props} value={value} onChange={setValue} />;
}

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

      <StateMatrix
        caption="Password field states"
        columns={['Rest', 'Hover', 'Focus', 'Filled', 'Invalid', 'Disabled']}
        rows={SIZES.map((size) => ({
          label: size.toUpperCase(),
          cells: [
            <Demo
              className="w-32"
              key="rest"
              label={hidden('Rest')}
              size={size}
              placeholder="Password"
            />,
            <Demo
              className="w-32"
              key="hover"
              label={hidden('Hover')}
              size={size}
              placeholder="Password"
              controlClassName={HOVER}
            />,
            <Demo
              className="w-32"
              key="focus"
              label={hidden('Focus')}
              size={size}
              initial="focused"
              controlClassName={FOCUS}
            />,
            <Demo
              className="w-32"
              key="filled"
              label={hidden('Filled')}
              size={size}
              initial="hunter2-value"
            />,
            <Demo
              className="w-32"
              key="invalid"
              label={hidden('Invalid')}
              size={size}
              initial="short"
              invalid
            />,
            <Demo
              className="w-32"
              key="disabled"
              label={hidden('Disabled')}
              size={size}
              initial="locked-value"
              disabled
            />,
          ],
        }))}
      />

      <Specimen
        title="Validation and variants"
        description="Pair two fields for confirmation, hide the toggle for write-only secrets, or localise the toggle's accessible names."
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
          description="Custom toggle labels."
          labels={{ show: 'Reveal phrase', hide: 'Conceal phrase' }}
          initial="correct horse battery"
        />
      </Specimen>

      <PropsTable
        rows={[
          {
            name: 'value / onChange',
            type: 'string / (value: string) => void',
            description: 'Always controlled: the field never keeps a copy of the secret.',
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
            type: '{ show?: string; hide?: string }',
            default: "'Show password' / 'Hide password'",
            description: 'Accessible names of the toggle.',
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
