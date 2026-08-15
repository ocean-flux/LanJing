import { Icon, type IconName, type IconProps } from '@/components/Icon';
import { cn } from '@/shared/utils';

/**
 * shadcn registry 组件（base-lyra）默认从 `@phosphor-icons/react` 导入图标。
 * 本项目图标统一走 Iconify，不引入该运行时依赖，因此这里提供同名替身，
 * 让 registry 组件只需替换一行 import 即可，便于后续 `shadcn add` 升级时重做。
 *
 * registry 里的尺寸选择器写的是 `[&_svg]:size-4` 这类，只匹配 svg，
 * 匹配不到本组件渲染的 span，因此这里给一个统一的默认字号兜底。
 */
type GlyphProps = Omit<IconProps, 'name'>;

function glyph(name: IconName) {
  return function Glyph({ className, ...props }: GlyphProps) {
    return <Icon name={name} className={cn('text-[0.875rem]', className)} {...props} />;
  };
}

export const CheckIcon = glyph('check');
export const XIcon = glyph('x');
export const CaretDownIcon = glyph('caret-down');
export const CaretUpIcon = glyph('caret-up');
export const CaretRightIcon = glyph('caret-right');
export const SidebarIcon = glyph('sidebar');
export const CheckCircleIcon = glyph('check-circle');
export const InfoIcon = glyph('info');
export const WarningIcon = glyph('warning');
export const XCircleIcon = glyph('x-circle');
export const SpinnerIcon = glyph('spinner');
