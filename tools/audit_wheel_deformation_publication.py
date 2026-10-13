"""Current-file complete-buffer audit sharing the unchanged publication audit."""
import validate_wheel_deformation_publication as validation
import audit_deformation_publication as common
if __name__=='__main__':
    validation.configure();common.main()
