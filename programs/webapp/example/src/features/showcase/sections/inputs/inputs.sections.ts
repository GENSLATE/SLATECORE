import type { ShowcaseSection } from '../../showcase.types';
import { CheckboxSection } from './checkbox.section';
import { NumberFieldSection } from './number-field.section';
import { PasswordFieldSection } from './password-field.section';
import { RadioGroupSection } from './radio-group.section';
import { SearchFieldSection } from './search-field.section';
import { SelectSection } from './select.section';
import { SliderSection } from './slider.section';
import { SwitchSection } from './switch.section';
import { TextFieldSection } from './text-field.section';
import { TextareaSection } from './textarea.section';

/** Showcase pages for the `inputs` category, in sidebar order. */
export const inputsSections: readonly ShowcaseSection[] = [
  {
    id: 'text-field',
    title: 'Text Field',
    description: 'Single-line input with label, helper, error, adornments and a clear button.',
    category: 'inputs',
    icon: 'codicon:symbol-string',
    covers: ['TextField', 'Field', 'FieldDescription', 'FieldError', 'FieldLabel'],
    component: TextFieldSection,
  },
  {
    id: 'password-field',
    title: 'Password Field',
    description:
      'A text field for secrets: masked until revealed, with no browser autofill or spell-check.',
    category: 'inputs',
    icon: 'codicon:key',
    covers: ['PasswordField'],
    component: PasswordFieldSection,
  },
  {
    id: 'search-field',
    title: 'Search Field',
    description: 'Search input with clear-on-Escape and a shortcut hint.',
    category: 'inputs',
    icon: 'codicon:search',
    covers: ['SearchField'],
    component: SearchFieldSection,
  },
  {
    id: 'textarea',
    title: 'Textarea',
    description: 'Multi-line text that can grow with its content.',
    category: 'inputs',
    icon: 'codicon:note',
    covers: ['Textarea'],
    component: TextareaSection,
  },
  {
    id: 'number-field',
    title: 'Number Field',
    description: 'Numeric input with a stacked stepper and keyboard stepping.',
    category: 'inputs',
    icon: 'codicon:symbol-numeric',
    covers: ['NumberField'],
    component: NumberFieldSection,
  },
  {
    id: 'select',
    title: 'Select',
    description:
      'A pop-up list with a checkmarked current value, grouped items and a bordered trigger.',
    category: 'inputs',
    icon: 'codicon:list-selection',
    covers: [
      'Select',
      'SelectGroup',
      'SelectGroupLabel',
      'SelectItem',
      'SelectPopup',
      'SelectSeparator',
      'SelectTrigger',
      'SelectValue',
    ],
    component: SelectSection,
  },
  {
    id: 'checkbox',
    title: 'Checkbox',
    description:
      'Crisp 14px checkboxes with an accent fill, a mixed state and groups with a select-all parent.',
    category: 'inputs',
    icon: 'codicon:pass',
    covers: ['Checkbox', 'CheckboxGroup'],
    component: CheckboxSection,
  },
  {
    id: 'radio-group',
    title: 'Radio Group',
    description: 'A single choice with roving arrow-key focus.',
    category: 'inputs',
    icon: 'codicon:circle-large-filled',
    covers: ['Radio', 'RadioGroup'],
    component: RadioGroupSection,
  },
  {
    id: 'switch',
    title: 'Switch',
    description: 'Flat on/off toggles with a sliding thumb, for settings that apply immediately.',
    category: 'inputs',
    icon: 'codicon:settings',
    covers: ['Switch'],
    component: SwitchSection,
  },
  {
    id: 'slider',
    title: 'Slider',
    description: 'Continuous or stepped values, single or range.',
    category: 'inputs',
    icon: 'codicon:settings-gear',
    covers: ['Slider'],
    component: SliderSection,
  },
];
