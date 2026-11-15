import {request} from './api';
import {activityQuery, type ActivityFilter, type ActivityPage} from './activityQuery';

/** Activity Log (backend activity_handlers.rs): who did what and when. */
export * from './activityQuery';
export const getActivity = (f: ActivityFilter) => request<ActivityPage>(`/admin/activity?${activityQuery(f)}`);

