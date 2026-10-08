import type { ShowcaseSection } from '../../showcase.types';
import { BadgeSection } from './badge.section';
import { BannerSection } from './banner.section';
import { EmptyStateSection } from './empty-state.section';
import { FeatureTeaserSection } from './feature-teaser.section';
import { ProgressSection } from './progress.section';
import { SkeletonSection } from './skeleton.section';

/** Showcase pages for the `feedback` category, in sidebar order. */
export const feedbackSections: readonly ShowcaseSection[] = [
  {
    id: 'badge',
    title: 'Badge',
    description: 'Compact labels and counts. Aurora tones only ever mean status.',
    category: 'feedback',
    icon: 'codicon:tag',
    covers: ['Badge'],
    component: BadgeSection,
  },
  {
    id: 'banner',
    title: 'Banner',
    description: 'Inline status messages with a tone icon, actions and dismiss.',
    category: 'feedback',
    icon: 'codicon:info',
    covers: ['Banner'],
    component: BannerSection,
  },
  {
    id: 'progress',
    title: 'Progress & Spinner',
    description: 'Thin progress bars and calm spinners.',
    category: 'feedback',
    icon: 'codicon:loading',
    covers: ['ProgressBar', 'Spinner'],
    component: ProgressSection,
  },
  {
    id: 'skeleton',
    title: 'Skeleton',
    description: 'Placeholders for content that is on its way.',
    category: 'feedback',
    icon: 'codicon:symbol-namespace',
    covers: ['Skeleton'],
    component: SkeletonSection,
  },
  {
    id: 'empty-state',
    title: 'Empty State',
    description: 'What to show when there is nothing to show — and a way forward.',
    category: 'feedback',
    icon: 'codicon:inbox',
    covers: ['EmptyState'],
    component: EmptyStateSection,
  },
  {
    id: 'feature-teaser',
    title: 'Feature teaser',
    description: 'Previews of features still to come, the same way in every app.',
    category: 'feedback',
    icon: 'codicon:sparkle',
    covers: ['FeatureTeaser', 'FeatureTeaserSample'],
    component: FeatureTeaserSection,
  },
];
