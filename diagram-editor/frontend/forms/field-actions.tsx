import {
  IconButton,
  InputAdornment,
  Menu,
  Stack,
  type SxProps,
  TextField,
  type TextFieldProps,
  Tooltip,
} from '@mui/material';
import { Templates, Widgets } from '@rjsf/mui';
import {
  type BaseInputTemplateProps,
  type TitleFieldProps,
  titleId,
  type WidgetProps,
} from '@rjsf/utils';
import {
  type ComponentType,
  createContext,
  type PropsWithChildren,
  type ReactNode,
  useContext,
  useState,
} from 'react';

import { MaterialSymbol } from '../nodes';

export function FieldMenu({
  label,
  title = label,
  symbol,
  disabled,
  menuListSx,
  children,
}: {
  label: string;
  title?: string;
  symbol: string;
  disabled?: boolean;
  menuListSx?: SxProps;
  children: () => ReactNode;
}) {
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  return (
    <>
      <Tooltip title={title}>
        <span>
          <IconButton
            size="small"
            aria-label={label}
            aria-haspopup="menu"
            disabled={disabled}
            onClick={(event) => setAnchor(event.currentTarget)}
          >
            <MaterialSymbol symbol={symbol} />
          </IconButton>
        </span>
      </Tooltip>
      <Menu
        anchorEl={anchor}
        open={Boolean(anchor)}
        disableScrollLock
        slotProps={{ list: { sx: menuListSx } }}
        onClose={() => setAnchor(null)}
        onClick={(event) => {
          event.stopPropagation();
          setAnchor(null);
        }}
      >
        {anchor && children()}
      </Menu>
    </>
  );
}

// A binding and a nullable value's menu belong to the same input, including
// when RJSF renders the input inside a variant or a registered type.
export const FieldActionContext = createContext<{
  id: string;
  actions: ReactNode;
} | null>(null);

export function FieldActions({
  id,
  actions,
  children,
}: PropsWithChildren<{ id: string; actions: ReactNode }>) {
  const inherited = useFieldActions(id);
  const controls = (
    <>
      {inherited}
      {actions}
    </>
  );
  return (
    <FieldActionContext.Provider value={{ id, actions: controls }}>
      {children}
    </FieldActionContext.Provider>
  );
}

export function useFieldActions(id: string) {
  const field = useContext(FieldActionContext);
  if (!field) return null;
  // RJSF gives the title and the alternative selector their own DOM ids.
  return [
    field.id,
    titleId(field.id),
    `${field.id}__oneof_select`,
    `${field.id}__anyof_select`,
  ].includes(id)
    ? field.actions
    : null;
}

const MuiInput =
  Templates.BaseInputTemplate as ComponentType<BaseInputTemplateProps>;
const MuiTitle = Templates.TitleFieldTemplate as ComponentType<TitleFieldProps>;

function useInputProps(
  id: string,
  props?: TextFieldProps['InputProps'],
  select = false,
) {
  const actions = useFieldActions(id);
  if (!actions) return props;
  return {
    ...props,
    endAdornment: (
      <>
        <InputAdornment position="end">{actions}</InputAdornment>
        {props?.endAdornment}
      </>
    ),
    ...(select && {
      sx: {
        display: 'grid',
        gridTemplateColumns: 'minmax(0, 1fr) auto',
        alignItems: 'center',
        '& .MuiSelect-select, & .MuiSelect-icon': { gridArea: '1 / 1' },
        '& .MuiSelect-icon': { position: 'static', justifySelf: 'end' },
      },
    }),
  };
}

export function ActionTextField(props: TextFieldProps) {
  return (
    <TextField
      {...props}
      fullWidth
      InputProps={useInputProps(props.id ?? '', props.InputProps)}
    />
  );
}

export function InputWithActions(props: BaseInputTemplateProps) {
  return (
    <MuiInput
      {...props}
      fullWidth
      InputProps={useInputProps(props.id, props.InputProps)}
    />
  );
}

export function SelectWithActions(props: WidgetProps) {
  return (
    <Widgets.SelectWidget
      {...props}
      fullWidth
      InputProps={useInputProps(props.id, props.InputProps, true)}
    />
  );
}

export function FieldRow({ id, children }: PropsWithChildren<{ id: string }>) {
  const actions = useFieldActions(id);
  return (
    <Stack
      direction="row"
      alignItems="center"
      justifyContent="space-between"
      sx={{ '& > .MuiFormControlLabel-root': { ml: 0 } }}
    >
      {children}
      {actions && <Stack direction="row">{actions}</Stack>}
    </Stack>
  );
}

export function CheckboxWithActions(props: WidgetProps) {
  return (
    <FieldRow id={props.id}>
      <Widgets.CheckboxWidget {...props} />
    </FieldRow>
  );
}

export function TitleWithActions(props: TitleFieldProps) {
  const actions = useFieldActions(props.id);
  if (!actions) return <MuiTitle {...props} />;
  return (
    <MuiTitle
      {...props}
      optionalDataControl={
        <>
          {props.optionalDataControl}
          {actions}
        </>
      }
    />
  );
}
