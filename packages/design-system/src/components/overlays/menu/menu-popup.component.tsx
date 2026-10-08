import { Menu as BaseMenu } from '@base-ui/react/menu';
import { popupSurface } from '../../../recipes';
import { cn } from '../../../utils/cn.util';
import type { MenuPopupProps } from './menu.types';
import { menuVariants } from './menu.variants';
import { useIsSubmenu } from './menu-context';

const styles = menuVariants();

/** Portal → Positioner → Popup on the flat floating-menu surface. Submenus overlap their trigger row. */
export function MenuPopup({
  side,
  align,
  sideOffset,
  alignOffset,
  className,
  ...props
}: MenuPopupProps) {
  const submenu = useIsSubmenu();
  return (
    <BaseMenu.Portal>
      <BaseMenu.Positioner
        side={side}
        align={align ?? 'start'}
        sideOffset={sideOffset ?? (submenu ? 2 : 4)}
        alignOffset={alignOffset ?? (submenu ? -5 : 0)}
        collisionPadding={8}
        className={styles.positioner()}
      >
        <BaseMenu.Popup
          data-slot="menu-popup"
          className={cn(popupSurface({ size: 'menu' }), styles.popup(), className)}
          {...props}
        />
      </BaseMenu.Positioner>
    </BaseMenu.Portal>
  );
}
