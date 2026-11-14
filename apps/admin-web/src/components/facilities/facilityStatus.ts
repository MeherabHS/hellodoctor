import type {PublicationStatus} from '../../lib/facilityContent';
import type {ChipTone} from '../ui/afya';

/** Status chip colour for facilities. */
export const FACILITY_CHIP:Record<PublicationStatus,ChipTone>={DRAFT:'warn',PUBLISHED:'success',ARCHIVED:'neutral'};
