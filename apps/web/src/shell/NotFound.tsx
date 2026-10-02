import { Link } from '@tanstack/react-router';

export function NotFound() {
    return (
        <div className="page">
            <div className="empty" role="alert">
                Not found. <Link to="/markets">Back to markets</Link>
            </div>
        </div>
    );
}
